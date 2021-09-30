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

import os
import sys

from botocore.exceptions import ClientError
import boto3


def main():
    keys = get_aws_info()

    subnet_info = None
    subnet_info = create_vpc(keys)
    rds = create_rds(subnet_info, keys)
    create_lambda(open_zipfile(), subnet_info, rds, keys)


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


def create_vpc(keys):
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

    vpc = res.create_vpc(CidrBlock="192.168.60.0/24")
    client.get_waiter("vpc_available").wait(
        Filters=[{
            "Name": "vpc-id",
            "Values": [vpc.id]
        }]
    )

    subnet = vpc.create_subnet(CidrBlock="192.168.60.240/28")
    iface = subnet.create_network_interface(
        Description="denViews iface."
    )

    inet_addr = client.allocate_address()
    inet_gate = client.create_internet_gateway()
    inet_gate_id = inet_gate["InternetGateway"]["InternetGatewayID"]
    res.InternetGateway(inet_gate_id).attach_to_vpc(VpcId=vpc.id)

    client.associate_address(
        AllocationId=inet_addr["AllocationId"],
        NetworkInterfaceId=iface.id
    )

    sec_group = list(vpc.security_groups.all())[0]
    client.get_waiter("security_group_exists").wait(
        Filters=[{
            "Name": "group-id",
            "Values": [sec_group.id]
        }]
    )

    sec_group.authorize_egress(
        IpPermissions=[
            {
                "IpProtocol": "-1",
                "Ipv6Ranges": [{"CidrIpv6": "::/0"}]
            }
        ]
    )

    return {
        "subnet_ids": [subnet.id],
        "security_groups": sec_group
    }


def create_rds(subnet_info, keys):
    client = service_client('rds', keys)

    try:
        client.describe_db_subnet_groups(
            DBSubnetGroupName="denviews-rds-subnet-group"
        )
        raise SetupError("denviews rds subnet group exists: aborting")
    except ClientError as err:
        if err.response["Error"]["Code"] == "DBSubnetGroupNotFoundFault":
            print("db subnet not found, continuing")
        else:
            raise err

    client.create_db_subnet_group(
        DBSubnetGroupName="denviews-rds-subnet-group",
        DBSubnetGroupDescription="denViews RDS subnet group.",
        SubnetIds=subnet_info["subnets"]
    )

    try:
        client.describe_db_instances(
            DBInstanceIdentifier="denviews"
        )
        raise SetupError("denviews rds instance exists: aborting")
    except ClientError as err:
        if err.response["Error"]["Code"] == "DBInstanceNotFoundFault":
            print("db instance not found, continuing")
        else:
            raise err

    rds = client.create_db_instance(
        DBName="denViews",
        DBInstanceIdentifier="denviews",
        AllocatedStorage=20,
        DBInstanceClass="db.t2.micro",
        Engine="mariadb",
        MasterUserName="denviews",
        MasterUserPassword="denviews",  # FIGURE SOMETHING OUT FOR THIS
        DBSubnetGroupName="denviews-rds-subnet-group",
        VpcSecurityGroupIds=list(map(
            lambda group: group.id,
            subnet_info["security_groups"]
        )),
        PubliclyAccessible=False,
        StorageType="gp2"
    )

    client.get_waiter('db_instance_available').wait(
        DBInstanceIdentifier="denviews"
    )

    return rds


def create_lambda(zipfile, subnet_info, rds, keys):
    client = service_client('lambda', keys)

    try:
        client.get_function(FunctionName="denViews")
        raise SetupError("denViews lambda already exists, aborting")
    except ClientError as err:
        if err.response['Error']['Code'] == 'ResourceNotFoundException':
            print("denViews lambda not found - uploading now")
        else:
            raise err

    client_iam = service_client('iam', keys)
    client_iam.create_role(
        RoleName="denviews-lambda",
        Description="denViews Lambda execution role."
    )
    client_iam.get_waiter('role_exists').wait(RoleName="denviews-lambda")
    role = resource_client('iam', keys).Role("denViews-lambda")
    role.attach_policy(PolicyArn="AWSLambdaBasicExecutionRole")
    role.attach_policy(PolicyArn="AWSXRayDaemonWriteAccess")

    lambda_func = client.create_function(
        FunctionName="denviews",
        Description="denViews by vulppine",
        Role=role.arn,
        Timeout=60,
        Runtime="provided.al2",
        Code={
            "ZipFile": zipfile
        },
        PackageType="Zip",
        Publish=True,
        VpcConfig={
            "SubnetIds": subnet_info["subnet_ids"],
            "SecurityGroupIds": list(map(
                lambda group: group.id,
                subnet_info["security_groups"]
            ))
        },
        Environment={
            "DENVIEWS_HOST": rds["DBInstance"]["Endpoint"]["Address"],
            "DENVIEWS_POOL_AMOUNT": "1",
        }
    )

    client.get_waiter('function_active').wait(
        FunctionName="denViews"
    )

    return lambda_func


def create_api(lambda_func, keys):
    client = service_client("apigatewayv2", keys)

    api = client.create_api(
        Name="denviews",
        ProtocolType="HTTP",
        CorsConfiguration={
            "AllowOrigins": ["*"],
        }
    )

    http_int = client.create_integration(
        ApiId=api["ApiId"],
        Description="HTML response for denViews.",
        IntegrationType="AWS",
        IntegrationUri=lambda_func["FunctionArn"],
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
        IntegrationUri=lambda_func["FunctionArn"],
        ResponseParameters={
            "200": {
                "overwrite:header.content-type": "application/json"
            }
        },
    )

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
