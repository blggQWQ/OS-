//! Standalone protocol bridge for the control-plane MODEL, not kernel control.
#[path = "../protection.rs"]
mod protection;
use protection::{Checked, Configuration, Mounted, Policy, Registry};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
fn raw() -> Configuration { Configuration { encrypted:false, integrity:false, read_only:false, root_hash:None, dependencies:vec![] } }
fn main() {
    let registry=Registry::new();
    registry.configure("data-01",raw()).unwrap();registry.configure("root-01",raw()).unwrap();
    let mut checked:HashMap<String,Checked>=HashMap::new();let mut mounted:HashMap<String,Mounted>=HashMap::new();
    for line in io::stdin().lock().lines() {
        let Ok(line)=line else {break};let a:Vec<&str>=line.split_whitespace().collect();
        let result:Result<String,String>=(||{
            let arg=|i:usize|a.get(i).copied().ok_or_else(||"BadRequest".to_string());
            match arg(0)? {
                "prepare"=>{
                    if a.len()!=9 { return Err("BadRequest".into()); }
                    let id=arg(1)?;let base=arg(2)?;if checked.contains_key(id)||mounted.contains_key(id){return Err("Duplicate".into());}
                    let bit=|i|->Result<bool,String>{match arg(i)?{"1"=>Ok(true),"0"=>Ok(false),_=>Err("BadRequest".into())}};
                    let crypt=bit(3)?;let verity=bit(4)?;let trusted=bit(5)?;
                    let config=Configuration{encrypted:crypt,integrity:verity,read_only:verity,root_hash:if verity{Some([7;32])}else{None},dependencies:vec![base.to_owned()]};
                    let policy=Policy{confidentiality:bit(6)?,integrity:bit(7)?,read_only:bit(8)?,expected_root:if trusted{Some([7;32])}else{None}};
                    let configured=registry.configure(id,config).map_err(|e|format!("{e:?}"))?;
                    match configured.verify(&policy){Ok(c)=>{checked.insert(id.into(),c);Ok("checked".into())},Err(e)=>{let _=registry.remove(id);Err(format!("{e:?}"))}}
                },
                "activate"=>{let id=arg(1)?;let c=checked.remove(id).ok_or("MissingReceipt")?;match c.activate(){Ok(m)=>{mounted.insert(id.into(),m);Ok("active".into())},Err(e)=>{let _=registry.remove(id);Err(format!("{e:?}"))}}},
                "change"=>{registry.configure(arg(1)?,raw()).map_err(|e|format!("{e:?}"))?;Ok("generation_changed".into())},
                "recreate"=>{let name=arg(1)?;registry.remove(name).map_err(|e|format!("{e:?}"))?;registry.configure(name,raw()).map_err(|e|format!("{e:?}"))?;Ok("identity_replaced".into())},
                "close"=>{let id=arg(1)?;if let Some(m)=mounted.remove(id){m.close();}else if checked.remove(id).is_none(){return Err("MissingReceipt".into());}registry.remove(id).map_err(|e|format!("{e:?}"))?;Ok("closed".into())},
                "pins"=>Ok(registry.pin_count(arg(1)?).map_err(|e|format!("{e:?}"))?.to_string()),
                _=>Err("BadRequest".into())
            }
        })();
        match result{Ok(s)=>println!("{{\"ok\":true,\"result\":\"{s}\"}}"),Err(e)=>println!("{{\"ok\":false,\"error\":\"{e}\"}}")};io::stdout().flush().unwrap();
    }
}
