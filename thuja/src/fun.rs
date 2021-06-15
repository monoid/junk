/*!
 Functional structures for fun and profit.
 */
use std::{borrow::Borrow, sync::Arc};

#[derive(Debug)]
pub struct Node<K, V> {
    key: K,
    val: V,
    less: Option<Arc<Node<K, V>>>,
    greater: Option<Arc<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    pub fn new(key: K, val: V) -> Self {
        Self {
            key,
            val,
            less: None,
            greater: None,
        }
    }
}

/// Quite dumb transient tree implementation.  Would work better with
/// some rebalancing.
#[derive(Debug, Clone)]
pub struct Tree<K, V> {
    root: Option<Arc<Node<K, V>>>,
}

impl<K, V> Default for Tree<K, V> {
    fn default() -> Self {
        Tree { root: None }
    }
}

impl<K, V> Tree<K, V> {
    pub fn new() -> Self {
        Default::default()
    }
}

enum Breadcrumb<'a, K, V> {
    Less(&'a Arc<Node<K, V>>),
    Greater(&'a Arc<Node<K, V>>),
}

impl<K, V> Tree<K, V>
where K: Ord + Eq + Clone,
      V: Clone {
    pub fn insert(&self, key: K, val: V) -> Self {
        let mut breadcrumbs = vec![];
        let mut node = &self.root;

        let mut target = Arc::new(loop {
            match node {
                Some(n) => {
                    match n.key.cmp(&key) {
                        std::cmp::Ordering::Less => {
                            breadcrumbs.push(Breadcrumb::Less(n));
                            node = &n.less;
                        }
                        std::cmp::Ordering::Greater => {
                            breadcrumbs.push(Breadcrumb::Greater(n));
                            node = &n.greater;
                        }
                        std::cmp::Ordering::Equal => {
                            break Node {
                                key: n.key.clone(),
                                val,
                                less: n.less.clone(),
                                greater: n.greater.clone(),
                            };
                        }
                    }
                }
                None => {
                    break Node::new(key, val);
                },
            }
        });
        
        for br in breadcrumbs.iter().rev() {
            target = Arc::new(match br {
                Breadcrumb::Less(n) => {
                    Node {
                        key: n.key.clone(),
                        val: n.val.clone(),
                        less: Some(target),
                        greater: n.greater.clone(),
                    }
                }
                Breadcrumb::Greater(n) => {
                    Node {
                        key: n.key.clone(),
                        val: n.val.clone(),
                        less: n.less.clone(),
                        greater: Some(target),
                    }
                }
            })
        }
        Tree { root: Some(target) }
    }

    // TODO relax key type
    // TODO: it doesn't need K: Clone and V: Clone
    pub fn get<'t, Q>(&'t self, key: &Q) -> Option<&'t V>
    where K: Borrow<Q>, Q: Ord + Eq + ?Sized {
        let mut node = &self.root;
        loop {
            match node {
                Some(n) => {
                    match n.key.borrow().cmp(key) {
                        std::cmp::Ordering::Less => {
                            node = &n.less;
                        }
                        std::cmp::Ordering::Greater => {
                            node = &n.greater;
                        }
                        std::cmp::Ordering::Equal => {
                            return Some(&n.val);
                        }
                    }
                }
                None => {
                    return None
                },
            }
        }
    }
}
