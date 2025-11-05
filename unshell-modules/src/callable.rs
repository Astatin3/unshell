// enum CallableArg {
//     Void,
//     String,
//     Int32,
//     Uint32,
// }

// enum CallableArgFilled {
//     String(String),
//     Int32(i32),
//     Uint32(u32),
// }

// struct Callable {
//     name: String,
//     args: Vec<CallableArg>,
//     return_type: CallableArg,
// }

// impl Callable {
//     fn new(name: String, args: Vec<CallableArg>) -> Self {
//         Callable { name, args }
//     }
//     fn call(&self, args: Vec<CallableArgFilled>, lib: &libloading::Library) -> Result<(), String> {
//         unsafe {
//             //TODO: Call the function with the given arguments
//             let func = lib.get::< (Find function type) >(self.name.as_bytes());
//             unsafe { func() };
//         }
//         Ok(())
//     }
// }
