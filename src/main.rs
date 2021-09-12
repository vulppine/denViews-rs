mod clients;
mod database;
mod util;

type Error = Box<dyn std::error::Error + Send + Sync>;

fn main() {
    /*
    if cfg!(feature = "hosted") {
        print_hosted();
    } else if cfg!(feature = "aws-lambda") {
        print_lambda();
    } else {
        println!("I do not have a valid feature flag set! Whoops!")
    }
    */

    print_type();
}

#[cfg(feature = "hosted")]
fn print_type() {
    println!("I am hosted as a continuously running server!");
}

#[cfg(feature = "aws-lambda")]
fn print_type() {
    println!("I am on AWS Lambda!");
}
