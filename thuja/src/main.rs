mod fun;
use std::{ops::Deref, sync::Arc};
use fun::Tree;

#[derive(Debug, Clone, Copy)]
struct Inode(u64);

#[derive(Debug, Clone)]
struct File {
    inode: Inode,
}

#[derive(Debug, Clone, Default)]
struct Dir {
    nodes: Tree<String, Arc<Node>>,
}

#[derive(Debug, Clone)]
enum Node {
    File(File),
    Dir(Dir),
}

impl Node {
    pub fn dir() -> Self {
        Node::Dir(Dir::default())
    }
}

#[derive(Debug, Clone)]
struct Fs {
    root: Dir,
}

#[derive(Debug)]
enum ThujaError {
    NotExists,
    AlreadyExists,
    IsFile,
}

impl Fs {
    fn new() -> Self {
        Fs {
            root: Dir {
                nodes: Default::default(),
            },
        }
    }

    fn mkdir(&mut self, base: &str, dirname: String) -> Result<Self, ThujaError> {
        let mut fs = self.clone();
        let mut dir = &fs.root;
        let mut breadcrumbs = vec![];

        for seg in base.split('/') {
            if ! seg.is_empty() {
                match dir.nodes.get(seg) {
                    Some(a) => {
                        match a.deref() {
                            Node::Dir(ref v) => {
                                breadcrumbs.push((seg, dir));
                                dir = v;
                            }
                            Node::File(_) => {
                                return Err(ThujaError::IsFile);
                            }
                        }
                    }
                    None => {
                        // Not found!
                        return Err(ThujaError::NotExists);
                    }
                }
            }
        }
        breadcrumbs.push((dirname.as_str(), dir));

        // unimplemented!("Checking that dirname does not exists");

        let mut node = Dir::default();
        for (name, dir) in breadcrumbs.iter().rev() {
            node = Dir {
                nodes: dir.nodes.insert(name.to_string(), Arc::new(Node::Dir(node)))
            };
        }
        Ok(Fs {
            root: node
        })
    }
}

fn main() {
    let mut fs = Fs::new();
    for dir in ["very", "long", "path"].iter() {
        fs = fs.mkdir("/", dir.to_string()).unwrap();
    }
    println!("{:?}", fs);

    fs = fs.mkdir("/long", "test".to_string()).unwrap();
    println!("{:?}", fs);
}
