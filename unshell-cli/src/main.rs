use std::io::{Write, stdin, stdout};

use unshell_lib::Announcement;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut serverruntime = unshell_lib::server::ListenerRuntime::new();

    loop {
        print!("> ");
        stdout().flush().expect("Failed to flush stdout");
        let mut input = String::new();
        stdin().read_line(&mut input).expect("Failed to read line");

        // println!("{}", input);

        let args = input.trim().split(" ").collect::<Vec<&str>>();

        match args[0] {
            "" => {}
            "test" => {
                if let Some(arg) = args.get(1) {
                    println!("Test with argument: {}", arg);
                    serverruntime
                        .send(&Announcement::TestAnnouncement(arg.to_string()))
                        .unwrap();
                } else {
                    println!("Test without argument");
                }
            }
            _ => {
                println!("Invalid Command: '{}'", args[0]);
            }
        }

        println!("{:?}", args);
    }
}
