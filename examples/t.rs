#[path="json/mod.rs"] mod json;
use crate::json::J as Value;
fn get<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for k in path { cur = cur.get(*k)?; }
    Some(cur)
}
fn main(){
  let src = std::fs::read_to_string("/data/dsh/home/ahd/work/web/fixtures.json").unwrap();
  let j = json::parse(&src).unwrap();
  let rows = get(&j, &["rows"]).and_then(|x| x.as_arr()).unwrap();
  for r in rows {
    if let Some(ls) = get(r, &["odds","ah"]).and_then(|x| x.as_arr()) {
      for l in ls {
        println!("cond={:?} H={:?} A={:?}", get(l,&["condition"]).and_then(|x|x.as_str()), get(l,&["odds","H"]).and_then(|x|x.as_str()), get(l,&["odds","A"]).and_then(|x|x.as_str()));
      }
      break;
    }
  }
}
