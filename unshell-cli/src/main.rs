use lazy_static::lazy_static;
use unshell_lib::{config::PayloadConfig, module::Manager};
use unshell_obfuscate::symbol;

lazy_static! {
    static ref PAYLOAD_CONFIG: PayloadConfig = PayloadConfig {
        id: symbol!("Test ID"),
        components: unshell_lib::get_components(),
        runtime_config: vec![],
    };
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unshell_lib::logger::PrettyLogger::init();

    Manager::run(&PAYLOAD_CONFIG, Vec::new());

    // let mut serverruntime = unshell_lib::server::ListenerRuntime::new();

    // loop {
    //     print!("> ");
    //     stdout().flush().expect("Failed to flush stdout");
    //     let mut input = String::new();
    //     stdin().read_line(&mut input).expect("Failed to read line");

    //     let args = input.trim().split(" ").collect::<Vec<&str>>();

    //     match args[0] {
    //         "" => {}
    //         "test" => {
    //             if let Some(arg) = args.get(1) {
    //                 println!("Test with argument: {}", arg);
    //                 serverruntime
    //                     .send(&Announcement::TestAnnouncement(arg.to_string()))
    //                     .unwrap();
    //             } else {
    //                 println!("Test without argument");
    //             }
    //         }
    //         _ => {
    //             println!("Invalid Command: '{}'", args[0]);
    //         }
    //     }

    //     // println!("{:?}", args);
    // }

    // serverruntime.send(&Announcement::GetRuntimes)?;

    // let response = serverruntime.

    Ok(())
}
