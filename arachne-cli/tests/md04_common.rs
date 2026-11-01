//! MD.04 oracle infrastructure only. The importer is always the real CLI process.
#![allow(dead_code)]
#[path = "../../../moirai-model-plane/moirai-interp/tests/nw02/launch.rs"]
mod launch;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const SCRATCH: &str = "/volatile/home/vp242103/.cache/cea-crdt-scratch/MD.04/work";
pub const NODE: &str = "/volatile/home/vp242103/.cache/cea-crdt-scratch/bin/model_node-748a185";
pub const NODE_SHA: &str = "3e06ebc3b034ee4632200ffd3582b628c4a4075ee629d141ab8b85ea592323bb";
pub const BAT_SHA: &str = "9d5af374c3823ee9ac439d21fb94023a566fab6f5dcbf3dc5e3b939eca86d505";
pub const LIMIT: Duration = Duration::from_secs(1800);
// Mutation arms patch exactly one of these constants. Ordinary runs are transparent.
const ARM_IL2: bool = false;
const ARM_DROP_LIBRARY: bool = false;
const ARM_DROP_ALIASES: bool = false;

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/md04")
        .join(name)
}
pub fn descriptor_path() -> PathBuf {
    root().join("demo/sysml.json")
}
pub fn read(path: &Path) -> Value {
    parse(
        &fs::read_to_string(path).unwrap_or_else(|e| panic!("MD04 setup {}: {e}", path.display())),
    )
}
pub fn hash(path: &Path) -> String {
    let o = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(o.status.success(), "MD04 setup hash {}", path.display());
    String::from_utf8(o.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}
pub fn batmobile() -> PathBuf {
    let p=std::env::var_os("MD04_BATMOBILE").map(PathBuf::from).unwrap_or_else(||PathBuf::from("/volatile/home/vp242103/.cache/cea-crdt-scratch/MD.02/census/source-2026-10-31/old/backend/application/syson-application-configuration/src/main/resources/templates/Batmobile.json"));
    assert_eq!(hash(&p), BAT_SHA, "MD04 setup pinned Batmobile");
    p
}
pub fn work(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let p = Path::new(SCRATCH).join(format!(
        "{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

// Lifted JSON parsing without adding a manifest feature. Split containers at their
// own commas, respecting quoted strings; serde_json still validates every leaf/key.
fn pieces(s: &str, delimiter: u8) -> Vec<&str> {
    let (mut depth, mut quoted, mut escape, mut start) = (0i32, false, false, 0);
    let mut out = Vec::new();
    for (i, b) in s.bytes().enumerate() {
        if quoted {
            if escape {
                escape = false
            } else if b == b'\\' {
                escape = true
            } else if b == b'"' {
                quoted = false
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                _ if b == delimiter && depth == 0 => {
                    out.push(s[start..i].trim());
                    start = i + 1
                }
                _ => {}
            }
        }
    }
    if !s[start..].trim().is_empty() {
        out.push(s[start..].trim())
    }
    out
}
pub fn parse(s: &str) -> Value {
    if let Ok(v) = serde_json::from_str(s) {
        return v;
    }
    let s = s.trim();
    if s.starts_with('{') && s.ends_with('}') {
        let mut m = serde_json::Map::new();
        for p in pieces(&s[1..s.len() - 1], b',') {
            let kv = pieces(p, b':');
            assert_eq!(kv.len(), 2, "MD04 setup JSON object");
            m.insert(serde_json::from_str::<String>(kv[0]).unwrap(), parse(kv[1]));
        }
        Value::Object(m)
    } else if s.starts_with('[') && s.ends_with(']') {
        Value::Array(
            pieces(&s[1..s.len() - 1], b',')
                .iter()
                .map(|s| parse(s))
                .collect(),
        )
    } else {
        panic!("MD04 setup invalid JSON: {}", &s[..s.len().min(200)])
    }
}

#[derive(Clone)]
pub struct Table {
    pub descriptor: Value,
    cache: Arc<Mutex<BTreeMap<String, BTreeMap<String, Value>>>>,
}
impl Table {
    pub fn new() -> Self {
        let descriptor = read(&descriptor_path());
        let digest = arachne_codegen::metamodel_digest(&descriptor);
        let expected = read(&fixture("pins.json"));
        assert_eq!(
            digest,
            expected["sysml_digest"].as_str().unwrap(),
            "MD04 setup shipped descriptor"
        );
        Self {
            descriptor,
            cache: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    pub fn features(&self, class: &str) -> BTreeMap<String, Value> {
        let cached = self.cache.lock().unwrap().get(class).cloned();
        if let Some(cached) = cached {
            return cached;
        }
        let c = &self.descriptor["classes"][class];
        assert!(c.is_object(), "MD04 setup unknown class {class}");
        let mut out = BTreeMap::new();
        for s in c["superTypes"].as_array().into_iter().flatten() {
            out.extend(self.features(s.as_str().unwrap()))
        }
        for kind in ["attributes", "containments", "references"] {
            for f in c[kind].as_array().into_iter().flatten() {
                out.insert(f["name"].as_str().unwrap().to_owned(), f.clone());
            }
        }
        self.cache
            .lock()
            .unwrap()
            .insert(class.to_owned(), out.clone());
        out
    }
    pub fn class(&self, e: &Value, fallback: &str) -> String {
        let name = e.get("eClass").and_then(Value::as_str).unwrap_or(fallback);
        if self.descriptor["classes"].get(name).is_some() {
            return name.to_owned();
        }
        let bare = name.rsplit(':').next().unwrap();
        if self.descriptor["classes"].get(bare).is_some() {
            return bare.to_owned();
        }
        let matches = self.descriptor["classes"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|n| n.ends_with(&format!("::{bare}")))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "MD04 setup class {name}");
        matches[0].clone()
    }
    pub fn class_at(&self, slot: u64) -> String {
        let sem = moirai_semantics::from_descriptor(&self.descriptor).unwrap();
        sem.classes[slot as usize].name.to_string()
    }
}
pub fn items(v: &Value) -> Vec<&Value> {
    match v {
        Value::Null => vec![],
        Value::Array(a) => a.iter().collect(),
        v => vec![v],
    }
}
pub fn data(v: &Value) -> &Value {
    v.get("data").unwrap_or(v)
}
// Independent recognition of the grammars in MD.03 section 1.1 and MD.03a
// section 1. No importer or runtime foreign-value recognizer is called.
pub fn library_value(text: &str) -> bool {
    fn identifier(text: &str) -> bool {
        let mut bytes = text.bytes();
        bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }
    fn uuid(text: &str) -> bool {
        let parts = text.split('-').collect::<Vec<_>>();
        parts.len() == 5
            && parts
                .iter()
                .zip([8, 4, 4, 4, 12])
                .all(|(part, n)| part.len() == n && part.bytes().all(|b| b.is_ascii_hexdigit()))
    }
    if let Some((class, address)) = text.split_once(' ') {
        if let Some((prefix, class)) = class.split_once(':') {
            if identifier(prefix) && identifier(class) {
                if let Some((scheme, coordinates)) = address.split_once(":///") {
                    if matches!(scheme, "sysmllibrary" | "kermllibrary") {
                        if let Some((resource, element)) = coordinates.split_once('#') {
                            if uuid(resource) && uuid(element) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut chars = text.chars().peekable();
    let mut segments = 0;
    loop {
        if chars.peek() == Some(&'\'') {
            chars.next();
            let mut nonempty = false;
            loop {
                match chars.next() {
                    Some('\'') if nonempty => break,
                    Some('\'') | None => return false,
                    Some('\\') => {
                        if chars.next().is_none() {
                            return false;
                        }
                        nonempty = true;
                    }
                    Some(_) => nonempty = true,
                }
            }
        } else {
            let mut segment = String::new();
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                segment.push(chars.next().unwrap());
            }
            if !identifier(&segment) {
                return false;
            }
        }
        segments += 1;
        match chars.next() {
            None => return segments >= 2,
            Some(':') if chars.next() == Some(':') => {}
            _ => return false,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dangling {
    pub holder: String,
    pub feature: String,
    pub value: String,
}
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Census {
    pub objects: usize,
    pub elements: usize,
    pub local: usize,
    pub library: usize,
    pub dangling: usize,
    pub aliases: usize,
    pub alias_values: usize,
    pub sysml: usize,
    pub kerml: usize,
}
#[derive(Clone)]
pub struct Source {
    pub project: Value,
    pub table: Table,
    pub by_source: BTreeMap<String, (String, String)>,
}
impl Source {
    pub fn new(path: &Path) -> Self {
        Self::from_value(read(path))
    }
    pub fn from_value(project: Value) -> Self {
        let table = Table::new();
        let mut s = Self {
            project,
            table,
            by_source: BTreeMap::new(),
        };
        fn collect(v: &Value, t: &Table, out: &mut BTreeMap<String, (String, String)>) {
            if let Some(id) = v.get("id").and_then(Value::as_str) {
                let eid = data(v)["elementId"]
                    .as_str()
                    .expect("MD04 setup source elementId");
                assert!(
                    out.insert(id.to_owned(), (eid.to_owned(), t.class(v, "")))
                        .is_none(),
                    "MD04 setup duplicate SysON id"
                )
            }
            match v {
                Value::Object(m) => m.values().for_each(|v| collect(v, t, out)),
                Value::Array(a) => a.iter().for_each(|v| collect(v, t, out)),
                _ => {}
            }
        }
        collect(&s.project, &s.table, &mut s.by_source);
        s
    }
    pub fn census(&self) -> Census {
        let mut c = Census::default();
        fn go(s: &Source, v: &Value, fallback: &str, c: &mut Census) {
            c.objects += 1;
            if v.get("id").is_some() {
                c.elements += 1
            }
            let class = s.table.class(v, fallback);
            let f = s.table.features(&class);
            for (k, val) in data(v).as_object().unwrap() {
                let decl = f
                    .get(k)
                    .unwrap_or_else(|| panic!("MD04 setup missing {class}.{k}"));
                match decl["merge"]["kind"].as_str().unwrap() {
                    "containment" => {
                        for x in items(val) {
                            go(s, x, decl["target"].as_str().unwrap(), c)
                        }
                    }
                    "identityReference" => {
                        for target in items(val) {
                            let target = target.as_str().unwrap();
                            if s.by_source.contains_key(target) {
                                c.local += 1
                            } else if library_value(target) {
                                c.library += 1;
                                if target.contains("sysmllibrary:///") {
                                    c.sysml += 1
                                } else if target.contains("kermllibrary:///") {
                                    c.kerml += 1
                                }
                            } else {
                                c.dangling += 1
                            }
                        }
                    }
                    _ => {
                        if k == "aliasIds" {
                            c.aliases += 1;
                            c.alias_values += items(val).len()
                        }
                    }
                }
            }
        }
        for r in self.project["content"].as_array().unwrap() {
            go(self, r, "", &mut c)
        }
        c
    }
    pub fn dangling(&self) -> Vec<Dangling> {
        fn go(s: &Source, src: &Value, fallback: &str, out: &mut Vec<Dangling>) {
            let class = s.table.class(src, fallback);
            let fields = s.table.features(&class);
            for (name, value) in data(src).as_object().unwrap() {
                let field = &fields[name];
                match field["merge"]["kind"].as_str().unwrap() {
                    "containment" => {
                        for child in items(value) {
                            go(s, child, field["target"].as_str().unwrap(), out);
                        }
                    }
                    "identityReference" => {
                        for target in items(value) {
                            let text = target.as_str().unwrap();
                            if !s.by_source.contains_key(text) && !library_value(text) {
                                out.push(Dangling {
                                    holder: src["id"]
                                        .as_str()
                                        .expect("dangling source SysON id")
                                        .into(),
                                    feature: name.clone(),
                                    value: text.into(),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for root in self.project["content"].as_array().unwrap() {
            go(self, root, "", &mut out);
        }
        out
    }
    pub fn check_dangling_report(&self, report: &str) {
        for missing in self.dangling() {
            let lines = report
                .lines()
                .filter(|line| {
                    let words = line.split_whitespace().collect::<Vec<_>>();
                    words.contains(&missing.holder.as_str())
                        && words.contains(&missing.feature.as_str())
                        && line.contains(&missing.value)
                })
                .count();
            assert_eq!(
                lines, 1,
                "MD04 G5: one report line required for {missing:?}: {report}"
            );
        }
    }
    /// Observe every object, including anonymous EAnnotations, by JSON path.
    pub fn all_identities(doc: &Value) -> BTreeMap<String, String> {
        fn go(v: &Value, path: &str, out: &mut BTreeMap<String, String>) {
            match v {
                Value::Object(m) => {
                    if m.contains_key("eClass") {
                        let id = m
                            .get("__id")
                            .and_then(Value::as_str)
                            .expect("MD04 claim: missing object identity");
                        out.insert(path.to_owned(), id.to_owned());
                    }
                    for (key, value) in m {
                        go(value, &format!("{path}/{key}"), out);
                    }
                }
                Value::Array(a) => {
                    for (i, value) in a.iter().enumerate() {
                        go(value, &format!("{path}/{i}"), out);
                    }
                }
                _ => {}
            }
        }
        let mut out = BTreeMap::new();
        go(doc, "", &mut out);
        out
    }
    pub fn served(doc: &Value) -> Result<BTreeMap<String, String>, String> {
        fn go(v: &Value, out: &mut BTreeMap<String, String>) -> Result<(), String> {
            match v {
                Value::Object(m) => {
                    if let Some(eid) = m.get("elementId").and_then(Value::as_str) {
                        if !eid.is_empty() {
                            let id = m
                                .get("__id")
                                .and_then(Value::as_str)
                                .ok_or_else(|| format!("no __id for {eid}"))?;
                            if out.insert(eid.to_owned(), id.to_owned()).is_some() {
                                return Err(format!("duplicate served elementId {eid}"));
                            }
                        }
                    }
                    for v in m.values() {
                        go(v, out)?
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        go(v, out)?
                    }
                }
                _ => {}
            }
            Ok(())
        }
        let mut out = BTreeMap::new();
        go(doc, &mut out)?;
        if out.values().collect::<BTreeSet<_>>().len() != out.len() {
            return Err("duplicate store identity".into());
        }
        Ok(out)
    }
    pub fn compare(
        &self,
        doc: &Value,
        identified: bool,
        structure: bool,
        ids: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        let mut count = 0;
        fn eq(a: &Value, b: &Value, at: &str) -> Result<(), String> {
            if a == b {
                Ok(())
            } else {
                Err(format!("{at}: expected {a}, read {b}"))
            }
        }
        fn go(
            s: &Source,
            src: &Value,
            got: &Value,
            fallback: &str,
            at: &str,
            identified: bool,
            structure: bool,
            ids: &BTreeMap<String, String>,
            count: &mut usize,
        ) -> Result<(), String> {
            *count += 1;
            let class = s.table.class(src, fallback);
            eq(&json!(class), &got["eClass"], at)?;
            if identified && got["__id"].as_str().is_none() {
                return Err(format!("{at}: missing __id"));
            }
            let fields = s.table.features(&class);
            for (name, value) in data(src).as_object().unwrap() {
                let f = fields
                    .get(name)
                    .ok_or_else(|| format!("unknown {class}.{name}"))?;
                let actual = &got[name];
                let here = format!("{at}.{name}");
                match f["merge"]["kind"].as_str().unwrap() {
                    "containment" => {
                        let a = items(value);
                        let b = items(actual);
                        if a.len() != b.len() {
                            return Err(format!(
                                "{here}: expected {} children, got {}",
                                a.len(),
                                b.len()
                            ));
                        }
                        for (i, (a, b)) in a.iter().zip(b).enumerate() {
                            go(
                                s,
                                a,
                                b,
                                f["target"].as_str().unwrap(),
                                &format!("{here}[{i}]"),
                                identified,
                                structure,
                                ids,
                                count,
                            )?
                        }
                    }
                    "identityReference" | "reference" => {
                        let identity = f["merge"]["kind"] == "identityReference";
                        if structure && identity {
                            if !actual.is_null() && actual != &json!([]) {
                                return Err(format!(
                                    "{here}: reference present before link phase: {actual}"
                                ));
                            }
                            continue;
                        }
                        let mut wanted = Vec::new();
                        for target in items(value) {
                            let text =
                                target.as_str().ok_or_else(|| format!("nonstring {here}"))?;
                            let target = match s.by_source.get(text) {
                                Some((eid, _)) => {
                                    if identity {
                                        ids.get(eid)
                                            .ok_or_else(|| format!("missing target {eid}"))?
                                            .clone()
                                    } else {
                                        eid.clone()
                                    }
                                }
                                None if identity && !library_value(text) => continue,
                                None => text.to_owned(),
                            };
                            wanted.push(json!(target));
                        }
                        if f["merge"]["many"] == true {
                            let mut actual = items(actual).into_iter().cloned().collect::<Vec<_>>();
                            wanted.sort_by_key(Value::to_string);
                            wanted.dedup();
                            actual.sort_by_key(Value::to_string);
                            eq(&json!(wanted), &json!(actual), &here)?
                        } else {
                            eq(wanted.first().unwrap_or(&Value::Null), actual, &here)?
                        }
                    }
                    "attribute" => {
                        let leaf = f["merge"]["leaf"]["kind"].as_str().unwrap_or("");
                        let expected = if leaf == "flag" {
                            value
                                .as_str()
                                .and_then(|s| s.parse::<bool>().ok())
                                .map(Value::Bool)
                                .unwrap_or(value.clone())
                        } else if leaf == "counter" {
                            value
                                .as_str()
                                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                                .unwrap_or(value.clone())
                        } else {
                            value.clone()
                        };
                        eq(&expected, actual, &here)?
                    }
                    kind => return Err(format!("{here}: source writes unsupported rule {kind}")),
                }
            }
            Ok(())
        }
        let roots = self.project["content"].as_array().unwrap();
        if roots.len() != 1 {
            return Err("source must have one root".into());
        }
        go(
            self, &roots[0], doc, "", "root", identified, structure, ids, &mut count,
        )?;
        fn object_count(v: &Value) -> usize {
            match v {
                Value::Object(m) => {
                    usize::from(m.contains_key("eClass"))
                        + m.values().map(object_count).sum::<usize>()
                }
                Value::Array(a) => a.iter().map(object_count).sum(),
                _ => 0,
            }
        }
        if object_count(doc) != count {
            return Err(format!(
                "expected {count} objects, got {}",
                object_count(doc)
            ));
        }
        Ok(())
    }
    pub fn check(
        &self,
        doc: &Value,
        identified: bool,
        structure: bool,
        ids: &BTreeMap<String, String>,
    ) {
        self.compare(doc, identified, structure, ids)
            .unwrap_or_else(|e| panic!("MD04 claim read-back: {e}"));
    }
    pub fn il2(&self, ids: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (eid, c) in self.by_source.values() {
            groups.entry(c.clone()).or_default().push(ids[eid].clone())
        }
        let mut out = BTreeMap::new();
        for g in groups.values_mut() {
            g.sort();
            g.dedup();
            for (i, id) in g.iter().enumerate() {
                out.insert(id.clone(), g[(i + 1) % g.len()].clone());
            }
        }
        out
    }
}

pub struct Response {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
}
pub fn http(port: u16, method: &str, path: &str, body: &str) -> Response {
    http_before(port, method, path, body, None)
        .unwrap_or_else(|e| panic!("MD04 setup HTTP {port} {method} {path}: {e}"))
}
fn http_before(
    port: u16,
    method: &str,
    path: &str,
    body: &str,
    deadline: Option<Instant>,
) -> io::Result<Response> {
    let remaining = || match deadline {
        Some(deadline) => deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "convergence deadline")),
        None => Ok(LIMIT),
    };
    let mut s = if deadline.is_some() {
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), remaining()?)?
    } else {
        TcpStream::connect(("127.0.0.1", port))?
    };
    s.set_read_timeout(Some(remaining()?))?;
    s.set_write_timeout(Some(remaining()?))?;
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut all = String::new();
    if deadline.is_some() {
        let mut bytes = Vec::new();
        let mut chunk = [0; 8192];
        loop {
            s.set_read_timeout(Some(remaining()?))?;
            let n = s.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..n]);
        }
        all =
            String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    } else {
        s.read_to_string(&mut all)?;
    }
    let (h, b) = all.split_once("\r\n\r\n").expect("MD04 setup HTTP header");
    let status = h
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = h
        .lines()
        .skip(1)
        .filter_map(|l| {
            l.split_once(':')
                .map(|(k, v)| (k.to_owned(), v.trim().to_owned()))
        })
        .collect::<Vec<_>>();
    let body = if headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("Transfer-Encoding") && v.eq_ignore_ascii_case("chunked")
    }) {
        let mut r = b;
        let mut out = String::new();
        loop {
            let (n, rest) = r.split_once("\r\n").unwrap();
            let n = usize::from_str_radix(n.split(';').next().unwrap(), 16).unwrap();
            if n == 0 {
                break;
            }
            out.push_str(&rest[..n]);
            r = &rest[n + 2..]
        }
        out
    } else {
        b.to_owned()
    };
    Ok(Response {
        status,
        body,
        headers,
    })
}
pub fn ok(port: u16, method: &str, path: &str, body: Value) -> Value {
    let r = http(
        port,
        method,
        path,
        if body.is_null() {
            "".into()
        } else {
            body.to_string()
        }
        .as_str(),
    );
    assert!(
        (200..300).contains(&r.status),
        "MD04 setup {method} {path}: {} {}",
        r.status,
        r.body
    );
    parse(&r.body)
}
pub fn state(port: u16, model: &str, identified: bool) -> String {
    let suffix = if identified { "/identified" } else { "" };
    let r = http(
        port,
        "GET",
        &format!("/api/model/{model}/state{suffix}"),
        "",
    );
    assert_eq!(r.status, 200, "MD04 setup state {}", r.body);
    r.body
}
// Only convergence reads may retry 504. Keep all other state reads fail-fast.
fn wait_for_convergence(ports: [u16; 2], model: &str, limit: Duration) -> (Duration, usize) {
    let start = Instant::now();
    let deadline = start + limit;
    let mut retries_504 = 0;
    let mut last_504 = None;
    loop {
        assert!(
            Instant::now() < deadline,
            "MD04 claim: replicas never converged; retries_504={retries_504}; last 504={last_504:?}"
        );
        let equal = [false, true].iter().all(|identified| {
            let suffix = if *identified { "/identified" } else { "" };
            let path = format!("/api/model/{model}/state{suffix}");
            let mut bodies = Vec::new();
            for port in ports {
                let r = http_before(port, "GET", &path, "", Some(deadline)).unwrap_or_else(|e| {
                    panic!(
                        "MD04 claim: convergence read {port} {path}: {e}; retries_504={retries_504}; last 504={last_504:?}"
                    )
                });
                if r.status == 504 {
                    retries_504 += 1;
                    last_504 = Some(format!("{port} {path}: {}", r.body));
                    return false;
                }
                assert_eq!(r.status, 200, "MD04 setup state {}", r.body);
                bodies.push(r.body);
            }
            bodies[0] == bodies[1]
        });
        assert!(
            Instant::now() < deadline,
            "MD04 claim: replicas never converged; retries_504={retries_504}; last 504={last_504:?}"
        );
        if equal {
            return (start.elapsed(), retries_504);
        }
        thread::sleep(
            Duration::from_millis(200).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
pub struct Cluster {
    pub nodes: Vec<launch::Launched>,
    pub dir: PathBuf,
    pub model: String,
    pub source: Source,
}
impl Cluster {
    pub fn new(source: Source, label: &str) -> Self {
        assert_eq!(hash(Path::new(NODE)), NODE_SHA, "MD04 setup node pin");
        let dir = work(label);
        let adapter = dir.join("loopback.so");
        let c = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../arachne-codegen/tests/fixtures/md08/md08_loopback.c");
        assert!(
            Command::new("cc")
                .args(["-shared", "-fPIC"])
                .arg(c)
                .args(["-ldl", "-o"])
                .arg(&adapter)
                .status()
                .unwrap()
                .success()
        );
        let mm = dir.join("metamodels");
        fs::create_dir(&mm).unwrap();
        fs::copy(descriptor_path(), mm.join("sysml.json")).unwrap();
        let spec = |name: &str| {
            launch::NodeSpec::new(
                name,
                &[
                    ("LOG_ID", "04040404040404040404040404040404"),
                    ("HTTP_BIND_ADDR", "127.0.0.1"),
                    ("MOIRAI_TRANSPORT", "tcp"),
                    ("METAMODEL_DIR", mm.to_str().unwrap()),
                    ("LD_PRELOAD", adapter.to_str().unwrap()),
                    ("TMPDIR", SCRATCH),
                    ("RUST_LOG", "warn"),
                ],
            )
        };
        let nodes = launch::launch(
            Path::new(NODE),
            &dir,
            &[spec("md04-alice"), spec("md04-bob")],
        )
        .unwrap_or_else(|e| panic!("MD04 setup launch: {e}"));
        let mut c = Self {
            nodes,
            dir,
            model: String::new(),
            source,
        };
        for node in &c.nodes {
            assert!(![7447, 7000, 7200, 8081, 8082, 8090, 8181, 8182, 8183].contains(&node.http));
            let deadline = Instant::now() + Duration::from_secs(40);
            loop {
                if let Ok(s) = TcpStream::connect(("127.0.0.1", node.http)) {
                    drop(s);
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "MD04 setup node HTTP {}",
                    node.id
                );
                thread::sleep(Duration::from_millis(30));
            }
            let v = ok(node.http, "GET", "/api/health", Value::Null);
            assert_eq!(v["replica_id"], node.id);
        }
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            let ready = c.nodes.iter().all(|node| {
                let peers = ok(node.http, "GET", "/api/peers", Value::Null);
                peers["peers"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|p| p["status"] == "Connected"))
            });
            if ready {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "MD04 setup: TCP mesh did not form"
            );
            thread::sleep(Duration::from_millis(50));
        }
        let key = arachne_codegen::metamodel_digest(&c.source.table.descriptor);
        let v = ok(
            c.port(0),
            "POST",
            "/api/models",
            json!({"metamodel_id":key}),
        );
        c.model = v["model_id"]
            .as_str()
            .expect("MD04 setup model id")
            .to_owned();
        ok(
            c.port(1),
            "POST",
            "/api/models",
            json!({"model_id":c.model}),
        );
        c
    }
    pub fn port(&self, i: usize) -> u16 {
        self.nodes[i].http
    }
    pub fn pause(&self) {
        for n in &self.nodes {
            ok(n.http, "POST", "/api/pause-all", Value::Null);
        }
    }
    pub fn resume(&self) {
        for n in &self.nodes {
            ok(n.http, "POST", "/api/resume-all", Value::Null);
        }
    }
    pub fn convergence(&self) {
        let (elapsed, retries_504) = wait_for_convergence(
            [self.port(0), self.port(1)],
            &self.model,
            Duration::from_secs(240),
        );
        println!(
            "MD04 convergence dir={} seconds={:.6} retries_504={retries_504}",
            self.dir.display(),
            elapsed.as_secs_f64()
        );
    }
    pub fn check(&self) -> BTreeMap<String, String> {
        let doc = parse(&state(self.port(0), &self.model, true));
        let ids = Source::served(&doc).unwrap();
        self.source.check(&doc, true, false, &ids);
        self.source.check(
            &parse(&state(self.port(0), &self.model, false)),
            false,
            false,
            &ids,
        );
        ids
    }
}
impl Drop for Cluster {
    fn drop(&mut self) {
        for n in &mut self.nodes {
            let _ = n.child.kill();
            let _ = n.child.wait();
        }
    }
}

fn has_link(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.contains_key("Link") || m.values().any(has_link),
        Value::Array(a) => a.iter().any(has_link),
        _ => false,
    }
}
fn link_values(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match v {
        Value::Object(m) => {
            if let Some(text) = m.get("Link").and_then(Value::as_str) {
                out.push(text.to_owned());
            }
            for value in m.values() {
                out.extend(link_values(value));
            }
        }
        Value::Array(a) => {
            for value in a {
                out.extend(link_values(value));
            }
        }
        _ => {}
    }
    out
}
fn mutate_links(v: &mut Value, map: &BTreeMap<String, String>) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(t)) = m.get_mut("Link") {
                if let Some(next) = map.get(t) {
                    *t = next.clone()
                }
            }
            for v in m.values_mut() {
                mutate_links(v, map)
            }
        }
        Value::Array(a) => {
            for v in a {
                mutate_links(v, map)
            }
        }
        _ => {}
    }
}
fn library_link(v: &Value) -> bool {
    match v {
        Value::Object(m) => {
            m.get("Link")
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains("sysmllibrary:///") || s.contains("kermllibrary:///"))
                || m.values().any(library_link)
        }
        Value::Array(a) => a.iter().any(library_link),
        _ => false,
    }
}
fn alias_op(v: &Value, t: &Table, mut class: Option<String>) -> bool {
    match v {
        Value::Object(m) => {
            if let Some(a) = m.get("Variant").and_then(Value::as_array) {
                class = Some(t.class_at(a[0].as_u64().unwrap()));
                return alias_op(&a[1], t, class);
            }
            if let Some(a) = m.get("Field").and_then(Value::as_array) {
                if let Some(c) = &class {
                    let f = t.features(c);
                    let name = f.keys().nth(a[0].as_u64().unwrap() as usize);
                    if name.is_some_and(|n| n == "aliasIds") {
                        return true;
                    }
                }
                return alias_op(&a[1], t, class);
            }
            m.values().any(|v| alias_op(v, t, class.clone()))
        }
        Value::Array(a) => a.iter().any(|v| alias_op(v, t, class.clone())),
        _ => false,
    }
}
#[derive(Default)]
pub struct Trace {
    pub identified_reads: usize,
    pub read_ids: BTreeMap<String, String>,
    pub first_seen: bool,
    pub forwarded_links: usize,
    pub dangling_writes: Vec<String>,
    pub error: Option<String>,
    pub il2: BTreeMap<String, String>,
}
pub struct Relay {
    pub port: u16,
    pub trace: Arc<Mutex<Trace>>,
    release: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Relay {
    pub fn new(c: &Cluster, hold: bool) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let trace = Arc::new(Mutex::new(Trace::default()));
        let release = Arc::new(AtomicBool::new(!hold));
        let stop = Arc::new(AtomicBool::new(false));
        let (tr, re, st) = (trace.clone(), release.clone(), stop.clone());
        let backend = c.port(0);
        let model = c.model.clone();
        let source = c.source.clone();
        let handle = thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => {
                        tr.lock().unwrap().error = Some(e.to_string());
                        break;
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(30)))
                    .unwrap();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut bytes = Vec::new();
                    let mut chunk = [0; 8192];
                    let (h, end) = loop {
                        let n = socket.read(&mut chunk).unwrap();
                        assert!(n > 0, "relay header EOF");
                        bytes.extend_from_slice(&chunk[..n]);
                        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            break (String::from_utf8(bytes[..i].to_vec()).unwrap(), i + 4);
                        }
                    };
                    let mut line = h.lines().next().unwrap().split_whitespace();
                    let method = line.next().unwrap();
                    let path = line.next().unwrap();
                    let length = h
                        .lines()
                        .find_map(|l| {
                            l.split_once(':')
                                .filter(|(k, _)| k.eq_ignore_ascii_case("Content-Length"))
                                .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    assert!(
                        !h.to_ascii_lowercase()
                            .contains("transfer-encoding: chunked"),
                        "MD04 setup relay needs fixed-length JSON bodies"
                    );
                    while bytes.len() < end + length {
                        let n = socket.read(&mut chunk).unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&chunk[..n])
                    }
                    let mut body = String::from_utf8(bytes[end..end + length].to_vec()).unwrap();
                    let is_ops = path == format!("/api/model/{model}/ops")
                        || path == format!("/api/model/{model}/op");
                    if is_ops {
                        let mut v = parse(&body);
                        let link = has_link(&v);
                        if link && !tr.lock().unwrap().first_seen {
                            let before = parse(&state(backend, &model, true));
                            let ids = Source::served(&before).unwrap();
                            let error = source.compare(&before, true, true, &ids).err();
                            let mut t = tr.lock().unwrap();
                            if t.identified_reads == 0 || t.read_ids != ids {
                                t.error = Some(
                                    "no complete matching identified read before first Link".into(),
                                )
                            }
                            if let Some(e) = error {
                                t.error = Some(e)
                            }
                            t.il2 = source.il2(&ids);
                            t.first_seen = true;
                            drop(t);
                            while !re.load(Ordering::SeqCst) && !st.load(Ordering::SeqCst) {
                                thread::sleep(Duration::from_millis(10))
                            }
                            if st.load(Ordering::SeqCst) {
                                return;
                            }
                        }
                        if ARM_IL2 {
                            mutate_links(&mut v, &tr.lock().unwrap().il2)
                        }
                        if ARM_DROP_LIBRARY || ARM_DROP_ALIASES {
                            if let Some(ops) = v.get_mut("ops").and_then(Value::as_array_mut) {
                                ops.retain(|op| {
                                    !(ARM_DROP_LIBRARY && library_link(op)
                                        || ARM_DROP_ALIASES && alias_op(op, &source.table, None))
                                });
                            } else if ARM_DROP_LIBRARY && library_link(&v)
                                || ARM_DROP_ALIASES && alias_op(&v, &source.table, None)
                            {
                                v = json!({"ops":[]});
                            }
                        }
                        if link {
                            let dangling = source.dangling();
                            tr.lock().unwrap().dangling_writes.extend(
                                link_values(&v).into_iter().filter(|value| {
                                    dangling.iter().any(|missing| &missing.value == value)
                                }),
                            );
                            tr.lock().unwrap().forwarded_links += 1
                        }
                        if ARM_IL2 || ARM_DROP_LIBRARY || ARM_DROP_ALIASES {
                            body = v.to_string()
                        }
                    }
                    let response = http(backend, method, path, &body);
                    if path == format!("/api/model/{model}/state/identified")
                        && response.status == 200
                    {
                        let document = parse(&response.body);
                        if let Ok(ids) = Source::served(&document) {
                            if source.compare(&document, true, true, &ids).is_ok() {
                                let mut trace = tr.lock().unwrap();
                                trace.identified_reads += 1;
                                trace.read_ids = ids;
                            }
                        }
                    }
                    write!(socket,"HTTP/1.1 {} MD04\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n",response.status,response.body.len()).unwrap();
                    for (k, v) in response.headers {
                        if k.eq_ignore_ascii_case("ETag") {
                            write!(socket, "{k}: {v}\r\n").unwrap()
                        }
                    }
                    let _ = write!(socket, "\r\n{}", response.body);
                }));
                if result.is_err() {
                    tr.lock().unwrap().error =
                        Some("relay I/O or parsing failed; see panic above".into());
                }
            }
        });
        Self {
            port,
            trace,
            release,
            stop,
            handle: Some(handle),
        }
    }
    pub fn release(&self) {
        self.release.store(true, Ordering::SeqCst)
    }
    pub fn assert_trace(&self) {
        let t = self.trace.lock().unwrap();
        assert!(
            t.error.is_none(),
            "MD04 claim phase ordering: {:?}",
            t.error
        );
        assert!(
            t.first_seen && t.identified_reads > 0 && t.forwarded_links > 0,
            "MD04 claim: no observed read/Link phase"
        );
    }
    pub fn barrier(&self, child: &mut Importer) {
        let until = Instant::now() + LIMIT;
        loop {
            let t = self.trace.lock().unwrap();
            if t.first_seen {
                assert!(
                    t.error.is_none(),
                    "MD04 claim phase ordering: {:?}",
                    t.error
                );
                assert_eq!(t.forwarded_links, 0);
                return;
            }
            if let Some(e) = &t.error {
                panic!("MD04 setup relay: {e}")
            }
            drop(t);
            if let Some(status) = child.child.try_wait().unwrap() {
                panic!(
                    "MD04 claim: importer exited {status} before first-link barrier: {}",
                    child.stderr()
                )
            }
            assert!(
                Instant::now() < until,
                "MD04 setup: first-link barrier timed out"
            );
            thread::sleep(Duration::from_millis(20))
        }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.release();
        if let Some(h) = self.handle.take() {
            h.join().unwrap();
        }
    }
}
pub struct Importer {
    pub child: Child,
    err: PathBuf,
    out: PathBuf,
}
impl Importer {
    pub fn start(c: &Cluster, path: &Path, port: u16, descriptor: &Path) -> Self {
        let dir = work("cli");
        let err = dir.join("stderr");
        let out = dir.join("stdout");
        let child = Command::new(env!("CARGO_BIN_EXE_arachne"))
            .arg("import-sirius")
            .arg(path)
            .arg("--descriptor")
            .arg(descriptor)
            .arg("--node")
            .arg(format!("http://127.0.0.1:{port}"))
            .arg("--model")
            .arg(&c.model)
            .env("TMPDIR", SCRATCH)
            .env("RUST_LOG", "error")
            .stdout(fs::File::create(&out).unwrap())
            .stderr(fs::File::create(&err).unwrap())
            .spawn()
            .unwrap();
        Self { child, err, out }
    }
    pub fn stderr(&self) -> String {
        fs::read_to_string(&self.err).unwrap_or_default()
    }
    pub fn finish(&mut self) -> std::process::ExitStatus {
        let until = Instant::now() + LIMIT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < until,
                "MD04 setup: importer exceeded resource timeout"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn success(&mut self) {
        let status = self.finish();
        assert!(
            status.success(),
            "MD04 claim live import: {status}\n{}",
            self.stderr()
        );
    }
    pub fn kill(&mut self) {
        self.child.kill().unwrap();
        let status = self.child.wait().unwrap();
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            status.signal(),
            Some(9),
            "MD04 setup actual SIGKILL required"
        )
    }
}
impl Drop for Importer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
