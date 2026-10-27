use serde_json::{Value, json};
use std::{fs, path::Path, thread, time::Duration};

pub fn handle(mode: &str, config: &Value, event: &Value, _count: u64) -> Option<Result<Value, String>> {
    let mut data = event["data"].clone();
    let name = event["event"].as_str().unwrap_or_default();
    let root = Path::new(event["workspace"].as_str().unwrap_or("."));
    Some(Ok(match mode {
        "echo-args" => json!({"output":data["arguments"]["text"]}),
        "constant-tool" => config.clone(),
        "compact-prompt" => {
            if name == "before_compact" {
                data["prompt"] = json!("CUSTOM COMPACTION PROMPT");
            }
            if name == "after_compact" {
                data["summary"] = json!(format!("{} [hook]", data["summary"].as_str().unwrap()));
            }
            data
        }
        "lifecycle" => {
            super::append(&root.join("events.log"), &format!("{name}\n"));
            match name {
                "system_prompt" => {
                    data["prompt"] = json!(format!("{}\nCUSTOM SYSTEM", data["prompt"].as_str().unwrap()))
                }
                "user_input" => {
                    data["content"] = json!(format!("{} [edited input]", data["content"].as_str().unwrap()))
                }
                "before_tool" => {
                    data["arguments"] = json!(json!({"path":"created.txt","content":"HOOK CONTENT"}).to_string())
                }
                "after_tool" => data["output"] = json!(format!("{} [edited result]", data["output"].as_str().unwrap())),
                "before_request" => data["body"]["temperature"] = json!(0.25),
                "after_response" if data["response"]["choices"][0]["finish_reason"] == "stop" => {
                    data["response"]["choices"][0]["message"]["content"] = json!("HOOK FINAL")
                }
                _ => {}
            }
            data
        }
        "orphan-history" => {
            data["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"role":"tool","tool_call_id":"orphan","content":"bad"}));
            data
        }
        "cross-origin" => {
            data["url"] = json!("https://other.invalid/chat/completions");
            data
        }
        "slow-tool" => {
            fs::write(root.join("started"), "ready").unwrap();
            thread::sleep(Duration::from_millis(800));
            json!({"output":"tool complete"})
        }
        "picture-tool" => {
            json!({"output":"external picture","images":[{"type":"image_url","image_url":{"url":"data:image/png;base64,iVBORw0KGgo="}}]})
        }
        "external-effect" => {
            fs::write("external-ran", "bad").unwrap();
            json!({"output":"executed"})
        }
        "approval-policy" => {
            assert_eq!(name, "tool_approval");
            let mut keys: Vec<&str> = data.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(keys, ["arguments", "decision", "external", "name"]);
            if config["reply"].is_null() {
                return Some(Err("policy unavailable".into()));
            }
            config["reply"].clone()
        }
        "mode-approval" => {
            let args: Vec<String> = std::env::args().collect();
            let index = args.iter().position(|arg| arg == "--mode-file").unwrap();
            let settings: Value = serde_json::from_slice(&fs::read(&args[index + 1]).unwrap()).unwrap();
            fs::write("policy-seen.json", json!({"mode":settings["mode"],"default":data["decision"]}).to_string())
                .unwrap();
            data["decision"] = json!("deny");
            data
        }
        "replacement-compact" => {
            super::append(&root.join("replacement.log"), &format!("{}\n", json!([name, std::process::id()])));
            match name {
                "compact_plan" => {
                    data["cut"] = json!(4);
                    data["state"] = json!({"chosen":"replacement"});
                }
                "before_compact" => {
                    assert_eq!(data["state"]["chosen"], "replacement");
                    assert_eq!(data["cut"], 4);
                    assert_eq!(data["messages"].as_array().unwrap().len(), 3);
                    data["summary"] = json!("LOCAL REPLACEMENT SUMMARY");
                    data["retained_images"] = json!([]);
                    data["state"]["prepared"] = json!(true);
                }
                "after_compact" => {
                    assert_eq!(data["state"]["prepared"], true);
                    data["summary"] = json!(format!("{} [custom postprocessing]", data["summary"].as_str().unwrap()));
                }
                _ => {}
            }
            data
        }
        "invalid-compact" => {
            let fault = config["mode"].as_str().unwrap();
            match name {
                "context_budget" => {
                    if fault == "missingbaseline" {
                        data.as_object_mut().unwrap().remove("content_estimate_tokens");
                    }
                    if fault == "unknownfits" {
                        data.as_object_mut().unwrap().remove("fits");
                    }
                }
                "compact_plan" => {
                    data["cut"] = match fault {
                        "zero" => json!(0),
                        "outside" => json!(999),
                        "tool" => json!(3),
                        "string" => json!("4"),
                        _ => json!(4),
                    }
                }
                "before_compact" => {
                    if fault == "fail" {
                        return Some(Err("replacement failed, do not retry".into()));
                    }
                    data["summary"] = match fault {
                        "empty" => json!(""),
                        "object" => json!({"tool_calls":[{"name":"write"}]}),
                        "expand" => json!("x".repeat(20000)),
                        "over_budget" => json!("x".repeat(50000)),
                        _ => json!("local"),
                    };
                }
                _ => {}
            }
            data
        }
        "provider" => return Some(provider(config, event)),
        _ => return None,
    }))
}

fn provider(config: &Value, event: &Value) -> Result<Value, String> {
    let data = &event["data"];
    let base = data["base_url"].as_str().unwrap_or_default();
    Ok(match config["kind"].as_str().unwrap() {
        "independent" => {
            assert!(std::env::var_os("PROVIDER_TEST_TOKEN").is_none());
            assert!(data.get("api_key").is_none() && data.get("key").is_none());
            assert_eq!(data["protocol"], "independent-vendor");
            assert_eq!(data["version"], 1);
            if event["event"] == "provider_prepare" {
                json!({"version":1,"url":format!("{base}/custom"),"body":{"question":data["history"].as_array().unwrap().last().unwrap()["content"],"cap":data["max_tokens"]},"headers":{"x-adapter":"custom"},"auth":{"header":"x-provider-key","prefix":"Key "}})
            } else {
                json!({"version":1,"response":{"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":data["response"]["answer"]}}]}})
            }
        }
        "fault" => {
            let fault = config["fault"].as_str().unwrap();
            if fault == "failure" {
                return Err("provider unavailable".into());
            }
            let mut result = json!({"version":1,"url":format!("{base}/custom"),"body":{},"headers":{},"auth":null});
            match fault {
                "origin" => result["url"] = json!("http://different.invalid/custom"),
                "header" => result["headers"] = json!({"Host":"different.invalid"}),
                "auth_name" => result["auth"] = json!({"header":"invalid header","prefix":""}),
                "auth_prefix" => result["auth"] = json!({"header":"x-key","prefix":"bad\nvalue"}),
                "auth_collision" => {
                    result["headers"] = json!({"X-Key":"shadow"});
                    result["auth"] = json!({"header":"x-key","prefix":""});
                }
                "version" => result["version"] = json!(2),
                _ => {}
            }
            result
        }
        "operation" => {
            use base64::Engine;
            let fault = config["fault"].as_str().unwrap();
            let url = if fault == "origin" { "http://other.invalid/aux".to_owned() } else { format!("{base}/aux") };
            let body = if fault == "oversized" {
                base64::engine::general_purpose::STANDARD.encode(vec![b'x'; 17 * 1024 * 1024])
            } else {
                "e30=".to_owned()
            };
            json!({"version":1,"state":{},"operation":{"url":url,"body_base64":body,"headers":{},"auth":null}})
        }
        "model" => json!({"version":1,"result":data["model"]}),
        "delete" => {
            let fault = config["mode"].as_str().unwrap();
            let url =
                if fault == "origin" { "http://other.invalid/danger".to_owned() } else { format!("{base}/danger") };
            let mut result = json!({"version":1,"state":{},"operation":{"url":url,"method":"DELETE","body_base64":"","headers":{},"auth":null}});
            match fault {
                "cleanup" => result["operation"]["cleanup"] = json!(true),
                "method" => result["operation"]["method"] = json!("PATCH"),
                "body" => result["operation"]["body_base64"] = json!("e30="),
                _ => {}
            }
            result
        }
        _ => return Err("unknown provider fixture".into()),
    })
}
