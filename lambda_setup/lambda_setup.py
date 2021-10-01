#!/usr/bin/python

# denViews AWS Lambda setup v0.1
#
# supports only creation with  MariaDB setup because
# i did this specifically for AWS Free Tier :eye:
#
# in order:
# - create the VPC
# - create the MariaDB RDS, link to VPC
# - create the Lambda function, link to VPC
# - create the API gateway
# - perform initial tasks on RDS, such as:
#   - creating denviews user
#   - setting global sql_mode NO_AUTO_VALUE_ON_ZERO
# - tell user to initialize denViews via browser
# - complete!

from requests import get
import json
import os
import sys

from botocore.exceptions import ClientError
import boto3
import mariadb


def main():
    print("!!! STARTING DENVIEWS AWS SETUP NOW !!!")
    keys = get_aws_info()

    check_service_existance(keys)

    ip_addr = get('https://api.ipify.org').text
    subnet_info = create_vpc(ip_addr, keys)
    rds = create_rds(subnet_info, keys)
    create_lambda(open_zipfile(), subnet_info, rds, keys)
    create_api(keys)

    print("!!! DENVIEWS AWS SETUP COMPLETE !!!")
    print("""
    Unfortunately, due to boto3 restrictions, you must
    set up the HTTP API integrations yourself.
    """)


def open_zipfile():
    with open(sys.argv[1], "rb") as zipfile:
        return zipfile.read()


def service_client(service, keys):
    return boto3.client(
        service,
        region_name=keys["region"],
        aws_access_key_id=keys["access_key"],
        aws_secret_access_key=keys["secret_key"]
    )


def resource_client(service, keys):
    return boto3.resource(
        service,
        region_name=keys["region"],
        aws_access_key_id=keys["access_key"],
        aws_secret_access_key=keys["secret_key"]
    )


def service_checker(service, error, **messages):
    try:
        service()
        print(messages["error_message"])
    except ClientError as err:
        if err.response["Error"]["Code"] == error:
            print(messages["success_message"])
        else:
            raise err


def check_service_existance(keys):
    print("checking if AWS services exist")
    # VPC check
    vpc_check = service_client("ec2", keys).describe_vpcs(
        Filters=[{"Name": "tag-key", "Values": ["denViews"]}]
    )
    if len(vpc_check["Vpcs"]) != 0:
        raise SetupError("denviews vpc exists: aborting")

    print("denViews VPC not found")

    # RDS CHECK
    rds_client = service_client("rds", keys)
    service_checker(
        lambda: rds_client.describe_db_subnet_groups(
            DBSubnetGroupName="denviews-rds-subnet-group"
        ),
        "DBSubnetGroupNotFoundFault",
        error_message="denviews rds db subnet found: aborting",
        success_message="denViews RDS subnet not found"
    )
    service_checker(
        lambda: rds_client.describe_db_instances(
            DBInstanceIdentifier="denviews"
        ),
        "DBInstanceNotFound",
        error_message="denviews rds instance exists: aborting",
        success_message="denViews RDS not found"
    )

    # LAMBDA CHECK
    service_checker(
        lambda: service_client("lambda", keys).get_function(
            FunctionName="denViews"
        ),
        "ResourceNotFoundException",
        error_message="denViews lambda already exists, aborting",
        success_message="denViews lambda not found"
    )

    # API GATEWAY CHECK
    service_checker(
        lambda: service_client("apigatewayv2", keys).get_api(ApiId="denviews"),
        "NotFoundException",
        error_message="denViews API already exists, aborting",
        success_message="denViews API does not exist"
    )


def create_vpc(ip, keys):
    """Creates the denViews VPC.

    This consists of creating a VPC labelled 'denviews-vpc',
    then creating a single subnet within 192.168.60.0/24.

    Afterwards, it creates a single interface, and then
    allocates a public-facing IP to it.

    It is much more cost-efficient to tie denViews to an existing
    VPC.
    """

    client = service_client('ec2', keys)
    res = resource_client('ec2', keys)

    print("creating vpc on 192.168.69.0/24")
    vpc = res.create_vpc(
        CidrBlock="192.168.69.0/24",
        TagSpecifications=[
            {
                "ResourceType": "vpc",
                "Tags": [{"Key": "denViews", "Value": "denViews"}]
            }
        ]
    )
    client.get_waiter("vpc_available").wait(
        Filters=[{
            "Name": "vpc-id",
            "Values": [vpc.id]
        }]
    )
    client.modify_vpc_attribute(
        EnableDnsHostnames={"Value": True},
        VpcId=vpc.id
    )

    print("creating VPC subnet 192.168.69.0/28")
    subnet_1 = vpc.create_subnet(
        CidrBlock="192.168.69.0/28",
        AvailabilityZone="us-east-2b")
    print("creating VPC subnet 192.168.69.64/28")
    subnet_2 = vpc.create_subnet(
        CidrBlock="192.168.69.64/28",
        AvailabilityZone="us-east-2c"
    )
    iface = subnet_1.create_network_interface(
        Description="denViews iface."
    )

    print("allocating elastic ip")
    inet_addr = client.allocate_address()
    inet_gate = client.create_internet_gateway()
    inet_gate_id = inet_gate["InternetGateway"]["InternetGatewayId"]
    res.InternetGateway(inet_gate_id).attach_to_vpc(VpcId=vpc.id)

    client.associate_address(
        AllocationId=inet_addr["AllocationId"],
        NetworkInterfaceId=iface.id
    )

    print("authorizing inbound/outbound traffic")
    sec_group = list(vpc.security_groups.all())[0]
    client.get_waiter("security_group_exists").wait(
        Filters=[{
            "Name": "group-id",
            "Values": [sec_group.id]
        }]
    )
    sec_group.authorize_ingress(
        IpPermissions=[{
            "IpProtocol": "TCP",
            "IpRanges": [{"CidrIp": ip + "/32"}],
            "FromPort": 3306,
            "ToPort": 3306
        }]
    )

    sec_group.authorize_egress(
        IpPermissions=[{
            "IpProtocol": "-1",
            "Ipv6Ranges": [{"CidrIpv6": "::/0"}]
        }]
    )

    route_table = list(vpc.route_tables.all())[0]
    route_table.create_route(
        DestinationCidrBlock="0.0.0.0/0",
        GatewayId=inet_gate_id
    )

    print("VPC setup completed")

    return {
        "subnet_ids": [subnet_1.id, subnet_2.id],
        "security_groups": [sec_group]
    }


def create_rds(subnet_info, keys):
    client = service_client('rds', keys)

    print("creating RDS db subnet group")
    client.create_db_subnet_group(
        DBSubnetGroupName="denviews-rds-subnet-group",
        DBSubnetGroupDescription="denViews RDS subnet group.",
        SubnetIds=subnet_info["subnet_ids"]
    )

    print("creating RDS - this may take a while")
    client.create_db_instance(
        DBName="denViews",
        DBInstanceIdentifier="denviews",
        AllocatedStorage=20,
        DBInstanceClass="db.t2.micro",
        Engine="mariadb",
        MasterUsername="denviews",
        MasterUserPassword="denviews",  # FIGURE SOMETHING OUT FOR THIS
        MultiAZ=False,
        DBSubnetGroupName="denviews-rds-subnet-group",
        VpcSecurityGroupIds=list(map(
            lambda group: group.id,
            iter(subnet_info["security_groups"])
        )),
        PubliclyAccessible=True,
        StorageType="gp2"
    )

    client.get_waiter('db_instance_available').wait(
        DBInstanceIdentifier="denviews"
    )

    rds = client.describe_db_instances(
        DBInstanceIdentifier="denviews"
    )

    setup_rds(rds["DBInstances"][0], "denviews")

    client.modify_db_instance(
        DBInstanceIdentifier="denviews",
        PubliclyAccessible=False
    )

    print("finalizing RDS setup")
    client.get_waiter('db_instance_available').wait(
        DBInstanceIdentifier="denviews"
    )

    print("RDS setup completed")
    return rds["DBInstances"][0]


def setup_rds(rds, password):
    print("initializing RDS for denViews use")
    conn = mariadb.connect(
        user=rds["MasterUsername"],
        password=password,
        host=rds["Endpoint"]["Address"],
        port=rds["Endpoint"]["Port"]
    )

    cur = conn.cursor()

    cur.execute("CREATE DATABASE denviews")

    conn.commit()
    conn.close()


def create_lambda(zipfile, subnet_info, rds, keys):
    client = service_client('lambda', keys)

    print("creating lambda role")
    client_iam = service_client('iam', keys)
    policy_doc = {
        "Version": "2012-10-17",
        "Statement": [
            {
                "Effect": "Allow",
                "Principal": {
                    "Service": "lambda.amazonaws.com"
                },
                "Action": "sts:AssumeRole"
            }
        ]
    }
    client_iam.create_role(
        RoleName="denviews-lambda",
        Description="denViews Lambda execution role.",
        AssumeRolePolicyDocument=json.dumps(policy_doc)
    )
    client_iam.get_waiter('role_exists').wait(RoleName="denviews-lambda")
    policy_tmpl = client_iam.get_policy(
        PolicyArn="arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
    )["Policy"]
    policy_tmpl_doc = client_iam.get_policy_version(
        PolicyArn=policy_tmpl["Arn"],
        VersionId=policy_tmpl["DefaultVersionId"]
    )["PolicyVersion"]["Document"]
    policy_tmpl_doc["Statement"][0]["Action"].extend([
        "ec2:DescribeInstances",
        "ec2:CreateNetworkInterface",
        "ec2:DeleteNetworkInterface",
        "ec2:AttachNetworkInterface",
        "ec2:DescribeNetworkInterfaces",
        "autoscaling:CompleteLifecycleAction"
    ])
    policy = client_iam.create_policy(
        PolicyName="denviews-lambda-policy",
        PolicyDocument=json.dumps(policy_tmpl_doc)
    )
    client_iam.get_waiter('policy_exists').wait(
        PolicyArn=policy["Policy"]["Arn"]
    )

    role = resource_client('iam', keys).Role("denViews-lambda")
    role.attach_policy(
         PolicyArn=policy["Policy"]["Arn"]
    )
    role.attach_policy(
        PolicyArn="arn:aws:iam::aws:policy/AWSXRayDaemonWriteAccess"
    )

    print("creating lambda function now")
    lambda_func = client.create_function(
        FunctionName="denviews",
        Description="denViews by vulppine",
        Role=role.arn,
        Timeout=60,
        Runtime="provided.al2",
        Handler="none",
        Code={
            "ZipFile": zipfile
        },
        PackageType="Zip",
        Publish=True,
        VpcConfig={
            "SubnetIds": [subnet_info["subnet_ids"][0]],
            "SecurityGroupIds": list(map(
                lambda group: group.id,
                subnet_info["security_groups"]
            ))
        },
        Environment={
            "Variables": {
                "DENVIEWS_HOST": rds["Endpoint"]["Address"],
                "DENVIEWS_POOL_AMOUNT": "1"
            }
        }
    )

    client.get_waiter('function_active').wait(
        FunctionName="denviews"
    )

    print("lambda setup completed")

    return lambda_func


def create_api(keys):
    client = service_client("apigatewayv2", keys)

    print("creating denViews API")
    api = client.create_api(
        Name="denviews",
        ProtocolType="HTTP",
        CorsConfiguration={
            "AllowOrigins": ["*"],
        }
    )

    # This doesn't work, due to AWS API limitations.
    '''
    http_int = client.create_integration(
        ApiId=api["ApiId"],
        Description="HTML response for denViews.",
        IntegrationType="AWS",
        IntegrationUri="arn:aws:lambda:us-east-2:468138320123:function:denviews",
        ResponseParameters={
            "200": {
                "overwrite:header.content-type": "text/html"
            }
        },
    )
    json_int = client.create_integration(
        ApiId=api["ApiId"],
        Description="JSON response for denViews.",
        IntegrationType="AWS",
        IntegrationUri="arn:aws:lambda:us-east-2:468138320123:function:denviews",
        ResponseParameters={
            "200": {
                "overwrite:header.content-type": "application/json"
            }
        },
    )
    '''

    client.create_route(
        ApiId=api["ApiId"],
        RouteKey="ANY /_denViews_dash",
    )
    client.create_route(
        ApiId=api["ApiId"],
        RouteKey="ANY /_denViews_dash/{proxy+}",
    )
    client.create_route(
        ApiId=api["ApiId"],
        RouteKey="ANY /_denViews_dash/api",
    )
    client.create_route(
        ApiId=api["ApiId"],
        RouteKey="POST /_denViews_flush",
    )

    print("API setup completed")


def get_aws_info():
    try:
        return {
            "access_key": os.environ["AWS_ACCESS_KEY"],
            "secret_key": os.environ["AWS_SECRET_KEY"],
            "region": os.environ["AWS_REGION"]
        }
    except KeyError as missing_key:
        print("A key is missing:", missing_key)
        sys.exit(1)


class SetupError(Exception):
    pass


if __name__ == "__main__":
    main()
