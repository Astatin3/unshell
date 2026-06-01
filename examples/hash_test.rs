macro_rules! hashtest {
    ($input:tt) => {
        ($input, unshell::hash_32!($input))
    };
}

const MAP: [(&str, u32); 6] = [
    hashtest!("abc123"),
    hashtest!("abc124"),
    hashtest!("abc125"),
    hashtest!("abc122"),
    hashtest!("somethingelse"),
    hashtest!("org.io.abc1234"),
];

pub fn main() {
    for (a, b) in MAP {
        println!("unshell::hash_32!(\"{}\") = {}", a, b)
    }
}
