use alloc::{
    fmt,
    string::{String, ToString},
    vec::Vec,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path(String);

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<&String> for Path {
    fn from(s: &String) -> Self {
        Self(s.clone())
    }
}

impl Path {
    pub const ROOT: &'static str = "/";
    pub const SEPARATOR: char = '/';
    pub const CURRENT_DIR: &'static str = ".";
    pub const PARENT_DIR: &'static str = "..";

    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn root() -> Self {
        Self::new(Self::ROOT.to_string())
    }

    pub fn normalize(&self) -> Self {
        let is_abs = self.is_abs();

        let parts = self
            .0
            .split(Self::SEPARATOR)
            .filter(|s| !s.is_empty() && *s != Self::CURRENT_DIR)
            .fold(Vec::new(), |mut acc, part| {
                if part == Self::PARENT_DIR {
                    if !acc.is_empty() && *acc.last().unwrap() != Self::PARENT_DIR {
                        acc.pop();
                    } else if is_abs {
                        // nothing to do
                    } else {
                        acc.push(part);
                    }
                } else {
                    acc.push(part);
                }
                acc
            });

        let normalized = if is_abs {
            format!("{}{}", Self::ROOT, parts.join(&Self::SEPARATOR.to_string()))
        } else {
            parts.join(&Self::SEPARATOR.to_string())
        };

        Self(normalized)
    }

    pub fn join(&self, name: &str) -> Self {
        let mut path = self.0.clone();
        if !path.ends_with(Self::SEPARATOR) {
            path.push(Self::SEPARATOR);
        }
        path.push_str(name);
        Self(path).normalize()
    }

    pub fn parent(&self) -> Self {
        let normalized = self.normalize().0;

        if normalized == Self::ROOT {
            return Self(normalized);
        }

        let mut parts: Vec<&str> = normalized
            .split(Self::SEPARATOR)
            .filter(|s| !s.is_empty())
            .collect();
        parts.pop();
        if self.0.starts_with(Self::ROOT) {
            let parent = format!("{}{}", Self::ROOT, parts.join(&Self::ROOT));
            if parent.is_empty() {
                Self::root()
            } else {
                Self(parent)
            }
        } else {
            let parent = parts.join(&Self::ROOT);
            if parent.is_empty() {
                Self(Self::CURRENT_DIR.to_string())
            } else {
                Self(parent)
            }
        }
    }

    pub fn names(&self) -> Vec<&str> {
        self.0
            .split(Self::SEPARATOR)
            .filter(|s| !s.is_empty())
            .collect()
    }

    pub fn name(&self) -> String {
        self.names().last().unwrap_or(&Self::ROOT).to_string()
    }

    pub fn is_abs(&self) -> bool {
        self.0.starts_with(Self::ROOT)
    }

    pub fn diff(&self, other: &Self) -> Self {
        if let Some(stripped) = self.0.strip_prefix(&other.0) {
            let stripped = stripped.strip_prefix(Self::SEPARATOR).unwrap_or(stripped);
            return Self(stripped.to_string());
        }

        if let Some(stripped) = other.0.strip_prefix(&self.0) {
            let stripped = stripped.strip_prefix(Self::SEPARATOR).unwrap_or(stripped);
            return Self(stripped.to_string());
        }

        self.clone()
    }
}

#[test_case]
fn test_new() {
    let path = Path::new("a/b/c");
    assert_eq!(path.to_string(), "a/b/c");
}

#[test_case]
fn test_root() {
    let path = Path::root();
    assert_eq!(path.to_string(), Path::ROOT);
}

#[test_case]
fn test_normalize() {
    let path = Path::new("/a/b/../c/.").normalize();
    assert_eq!(path.to_string(), "/a/c");
    let path = Path::new("a/b/../../c").normalize();
    assert_eq!(path.to_string(), "c");
    let path = Path::new("/a/./b/c/../d").normalize();
    assert_eq!(path.to_string(), "/a/b/d");
    let path = Path::new("hoge.txt").normalize();
    assert_eq!(path.to_string(), "hoge.txt");
}

#[test_case]
fn test_join() {
    let path = Path::new("/a/b").join("c");
    assert_eq!(path.to_string(), "/a/b/c");
    let path = Path::new("/a/b/").join("c");
    assert_eq!(path.to_string(), "/a/b/c");
    let path = Path::new("a/b").join("c");
    assert_eq!(path.to_string(), "a/b/c");
}

#[test_case]
fn test_parent() {
    let path = Path::new("/a/b/c").parent();
    assert_eq!(path.to_string(), "/a/b");
    let path = Path::new("/a".to_string()).parent();
    assert_eq!(path.to_string(), "/");
    let path = Path::new("a/b/c").parent();
    assert_eq!(path.to_string(), "a/b");
    let path = Path::new("a").parent();
    assert_eq!(path.to_string(), ".");
}

#[test_case]
fn test_names() {
    let path = Path::new("/a/b/c");
    let names: Vec<&str> = path.names();
    assert_eq!(names, vec!["a", "b", "c"]);
    let path = Path::new("a/b/c");
    let names: Vec<&str> = path.names();
    assert_eq!(names, vec!["a", "b", "c"]);
}

#[test_case]
fn test_name() {
    let path = Path::new("/a/b/c");
    assert_eq!(path.name(), "c");
    let path = Path::new("a/b/c");
    assert_eq!(path.name(), "c");
}

#[test_case]
fn test_is_abs() {
    let path = Path::new("/a/b/c");
    assert!(path.is_abs());
    let path = Path::new("a/b/c");
    assert!(!path.is_abs());
}

#[test_case]
fn test_diff() {
    let path1 = Path::new("/a/b/c");
    let path2 = Path::new("/a/b");
    assert_eq!(path1.diff(&path2).to_string(), "c");
    assert_eq!(path2.diff(&path1).to_string(), "c");
    let path3 = Path::new("/a/b/c/d");
    assert_eq!(path1.diff(&path3).to_string(), "d");
}

#[test_case]
fn test_relative_paths() {
    let path = Path::new("a/b/../../c").normalize();
    assert_eq!(path.to_string(), "c");
    let path = Path::new("/a/b/../../../..").normalize();
    assert_eq!(path.to_string(), "/");
    let path = Path::new("../a/b/../c").normalize();
    assert_eq!(path.to_string(), "../a/c");
    let path = Path::new("../../a/b/c").normalize();
    assert_eq!(path.to_string(), "../../a/b/c");
}
