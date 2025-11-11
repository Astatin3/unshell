use std::io::{Write, stdin, stdout};

use unshell_crypt::{
    aes::{decrypt_aes, decrypt_aes_lines, encrypt_aes, encrypt_aes_lines},
    base62::Base62,
    fill, hash,
};
use unshell_lib::Announcement;

use unshell_obfuscate::format_obs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let aaa = unshell_lib::crypt::aes::decrypt_aes(
    //     "611dcb046fcb11c3a0cf6d27ac7c8452e0120c2675d067a0dd857d9cd3d9df21140f1e2a715083a48180907eb90b87a6", //_1c82a871dda0f4372eb8e0dbba34c8de",
    //     "abc123abc",
    // )?;

    let key = "abc123abc";

    println!(
        "{}",
        encrypt_aes_lines("Verylongstringthat1", &key, unshell_crypt::STATIC_IV)
    );
    println!(
        "{}",
        encrypt_aes_lines("Verylongstringthat12", &key, unshell_crypt::STATIC_IV)
    );
    println!(
        "{}",
        encrypt_aes_lines("Verylongstringthat123", &key, unshell_crypt::STATIC_IV)
    );
    println!(
        "{}",
        encrypt_aes_lines("Verylongstringthat1234", &key, unshell_crypt::STATIC_IV)
    );
    println!(
        "{}",
        encrypt_aes_lines("Verylongstringthat12345", &key, unshell_crypt::STATIC_IV)
    );

    // let e = Base62::encode_full(data, &key);
    // let e = "_Nl8MlCFOyM4egPSXfo0wE4tfA0vOfBSZCx1TpKzjhI2qfahTrwh4JE1pJpcBttQqz_";

    // let mut buf = [0u8; 256];
    // _L1IuRLMW8tZN68RerKcbltQl675Yeq930NbcxsEfYYf_
    // _L1IuRLMW8tZN68RerKcblyGtqOCLeLuBu3ormgklt3J_
    // _L1IuRLMW8tZN68RerKcblKi37HPcZRonvwFUS5SZc0C_
    // _L1IuRLMW8tZN68RerKcblya0Yw6TrFQxruqpemtv3K2_

    // fill(&mut buf);

    // for byte in buf.iter() {
    //     print!("{}, ", byte);
    // }

    // src/main.rs:13
    // _SqF7lDRCyatsM4hnUTNAOq_
    //
    // _Nl8MlCFOyM4egPSXfo0wE4tfA0vOfBSZCx1TpKzjhI2qfahTrwh4JE1pJpcBttQqz_
    // unshell-lib-0.0.0/src/module/manager.rs:52

    // println!("Key: {}", String::from_utf8_lossy(&key));

    // let encoded = Base62::decode_full(&e, &key).unwrap();
    // let plaintext = decrypt_aes_lines(&data, &key, unshell_crypt::STATIC_IV);

    // let base = Base62::new(&hash("TEST_KEY".as_bytes()), 0);

    // let encoded = base.encode("123 1234 12342 1235e2".as_bytes());

    // println!("Base62: {}", plaintext);

    // let base = Base62::new(&hash("TEST_KEY".as_bytes()), 0);

    // let decoded = base.decode(&encoded).unwrap();
    // println!("Decoded: {}", String::from_utf8(decoded).unwrap());

    // Ok(())

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
