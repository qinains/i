use i_wasm_sdk::*;
#[derive(Default)]
pub struct WordCount;
impl Plugin for WordCount {
    fn start(&mut self,request:ToolRequest)->StepOutput {
        if request.data.name!="count_words" {return StepOutput::Error{error:"unknown tool".into()};}
        match request.data.arguments["path"].as_str(){Some(path)=>host_read(1,path),None=>StepOutput::Error{error:"path required".into()}}
    }
    fn resume(&mut self,response:HostResponse)->StepOutput {
        if !response.ok{return StepOutput::Error{error:response.error.unwrap_or_else(||"read failed".into())};}
        let result=response.result.unwrap_or(Value::Null);
        match result["text"].as_str(){Some(text)=>StepOutput::Done{output:json!({"words":text.split_whitespace().count(),"characters":text.chars().count()}).to_string()},None=>StepOutput::Error{error:"invalid text response".into()}}
    }
}
export_plugin!(crate::WordCount);
