use std::collections::VecDeque;

pub struct Node<T> {
    pub val: T,
    pub left: Option<Box<Node<T>>>,
    pub right: Option<Box<Node<T>>>,
}

impl<T> From<T> for Node<T> {
    fn from(val: T) -> Self {
        Self { val, left: None, right: None }
    }
}

pub fn rotate_in_place_deq<T>(root: &mut Node<T>) {
    let mut deq = VecDeque::new();
    deq.push_back(root);
    while let Some(node) = deq.pop_front() {
        std::mem::swap(&mut node.left, &mut node.right);
        if let Some(l) = node.left.as_mut() {
            deq.push_back(l);
        }
        if let Some(r) = node.right.as_mut() {
            deq.push_back(r);
        }
    }
}

pub fn rotate_in_place_vec<T>(root: &mut Node<T>) {
    let mut stack = Vec::new();
    let mut top = Some(root);

    while let Some(node) = top.take().or_else(|| stack.pop()) {
        std::mem::swap(&mut node.left, &mut node.right);

        if let Some(l) = node.left.as_mut() {
            top = Some(l);
        }
        if let Some(r) = node.right.as_mut() {
            if let Some(p) = top.take() {
                stack.push(p);
            }
            top = Some(r);
        }
    }
}
