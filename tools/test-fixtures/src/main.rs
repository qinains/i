use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, Write},
    path::Path,
    thread,
    time::Duration,
};

mod pty;
mod scenarios;

fn append(path: &Path, text: &str) {
    use std::fs::OpenOptions;
    OpenOptions::new().create(true).append(true).open(path).unwrap().write_all(text.as_bytes()).unwrap();
}

fn handle(mode: &str, config: &Value, event: &Value, count: u64) -> Result<Value, String> {
    let data = &event["data"];
    Ok(match mode {
        "analysis-filter" => {
            let serialized = data.to_string();
            assert!(data.get("api_key").is_none());
            for denied in config["denied"].as_array().unwrap() {
                assert!(!serialized.contains(denied.as_str().unwrap()));
            }
            if config["phases"] == true {
                assert!(data.get("history").is_none());
                if event["event"] == "analysis_prepare" {
                    assert_eq!(data["transcript"]["messages"][0]["role"], "user");
                    json!({"summary":"local reflection","state":{"prepared":true}})
                } else {
                    assert_eq!(event["event"], "analysis_result");
                    assert_eq!(data["state"]["prepared"], true);
                    json!({"summary":format!("{} checked",data["summary"].as_str().unwrap())})
                }
            } else {
                json!({"summary":config["summary"]})
            }
        }
        "analysis-policy" => {
            if event["event"] == "analysis_prepare" {
                json!({"prompt":"Review the evidence; no actions"})
            } else {
                data.clone()
            }
        }
        "analysis-provider" | "connection-provider" => {
            assert_eq!(event["event"], "provider_complete");
            assert!(data.get("api_key").is_none());
            if mode == "analysis-provider" {
                assert_eq!(data["compact"], true);
                assert_eq!(data["tools"], json!([]));
                assert_eq!(data["plugin_tools"], json!([]));
                assert!(!data.to_string().contains("HOST_SECRET"));
            }
            let reason = config["reason"].as_str().unwrap_or("stop");
            let mut message = json!({"role":"assistant","content":config["content"]});
            if reason == "tool_calls" {
                message["tool_calls"] =
                    json!([{"id":"blocked","type":"function","function":{"name":"write","arguments":"{}"}}]);
            }
            json!({"version":1,"response":{"choices":[{"finish_reason":reason,"message":message}]}})
        }
        "pool" => json!({"pid":std::process::id(),"n":count,"cwd":std::env::current_dir().unwrap()}),
        "effect-error" => {
            append(Path::new("effects"), &format!("{}\n", std::process::id()));
            return Err("side effect already happened".into());
        }
        "append-tool" => {
            append(&Path::new(event["workspace"].as_str().unwrap()).join("calls.txt"), "once\n");
            json!({"output":"done"})
        }
        _ => {
            return scenarios::handle(mode, config, event, count).unwrap_or_else(|| Err("unknown fixture mode".into()));
        }
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).expect("fixture mode required");
    if mode == "console-menu-pty" {
        pty::run(args.get(2).expect("console binary required"));
        return;
    }
    let config: Value = serde_json::from_str(args.get(2).map(String::as_str).unwrap_or("{}")).unwrap();
    match mode.as_str() {
        "invalid-json" => {
            println!("not json");
            return;
        }
        "error-exit" => {
            eprintln!("fixture failure");
            std::process::exit(1);
        }
        "stderr-flood" => {
            io::stderr().write_all(&vec![b'x'; 30000]).unwrap();
            io::stderr().flush().unwrap();
            thread::sleep(Duration::from_secs(30));
            return;
        }
        "block-stdin" => {
            thread::sleep(Duration::from_millis(700));
            fs::write("late-effect", "bad").unwrap();
            return;
        }
        _ => {}
    }
    for (index, line) in io::stdin().lock().lines().enumerate() {
        let event: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let id = event["id"].clone();
        let reply = match mode.as_str() {
            "mismatch" => json!({"id":id.as_u64().unwrap()+1,"data":{}}),
            "nonobject" => json!({"id":id,"data":[]}),
            "oversize" => {
                io::stdout().write_all(&vec![b'x'; 48 * 1024 * 1024 + 1]).unwrap();
                io::stdout().flush().unwrap();
                return;
            }
            "eof" => return,
            _ => match handle(mode, &config, &event, index as u64 + 1) {
                Ok(data) => json!({"id":id,"data":data}),
                Err(error) => json!({"id":id,"error":error}),
            },
        };
        println!("{reply}");
        io::stdout().flush().unwrap();
    }
}
