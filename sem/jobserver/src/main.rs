#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    jobserver::main()
}
