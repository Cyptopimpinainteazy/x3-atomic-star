use x3_readiness_report::{Collector, JsonFormatter, TextFormatter};

fn main() {
    let mut offline = false;
    let mut json = false;
    let mut rpc_override: Option<String> = None;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--offline" => offline = true,
            "--json" => json = true,
            "--text" => json = false,
            "--rpc" => {
                if let Some(v) = iter.next() {
                    rpc_override = Some(v);
                }
            }
            _ => {}
        }
    }

    let report = if offline {
        Collector::collect_offline()
    } else {
        Collector::collect_live(rpc_override.as_deref())
    };

    if json {
        println!("{}", JsonFormatter::format(&report));
    } else {
        println!("{}", TextFormatter::format(&report));
    }
}
