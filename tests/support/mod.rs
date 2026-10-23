// Test-only adapter: concise request handlers run inside a framed worker fixture.
#![allow(dead_code)]
use std::{path::Path, process::Command};
pub fn python(body: &str) -> String {
    format!(
        r#"import io,json,sys
source={}
wire_in,wire_out=sys.stdin,sys.stdout
for line in wire_in:
    frame=json.loads(line)
    ident=frame.pop('id')
    captured=io.StringIO()
    sys.stdin,sys.stdout=io.StringIO(json.dumps(frame)),captured
    try:
        exec(source,{{'__name__':'__main__'}})
        reply={{'id':ident,'data':json.loads(captured.getvalue())}}
    except Exception as error:
        reply={{'id':ident,'error':str(error)}}
    finally:
        sys.stdin,sys.stdout=wire_in,wire_out
    wire_out.write(json.dumps(reply)+'\n')
    wire_out.flush()
"#,
        serde_json::to_string(body).unwrap()
    )
}
pub fn isolate(command: &mut Command, root: &Path) {
    for key in [
        "API_KEY",
        "DEEPSEEK_API_KEY",
        "DEEPSEEK_TEST_API_KEY",
        "OPENAI_API_KEY",
        "I_TEST_UNSET_KEY",
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        command.env_remove(key);
    }
    for (key, name) in
        [("I_CONFIG_DIR", "config"), ("I_DATA_DIR", "data"), ("I_CACHE_DIR", "cache"), ("I_RUNTIME_DIR", "runtime")]
    {
        command.env(key, root.join(".app").join(name));
    }
}
pub fn store(root: &Path) -> std::path::PathBuf {
    let path = root.join(".app/data/workspaces");
    std::fs::read_dir(path).unwrap().next().unwrap().unwrap().path()
}
