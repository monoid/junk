/// Conceptual implementation of interned weak string pool.
///
/// The pool interns &str, and returns Rc<PooledStr>>, and PooledStr derefs to &str.
///
/// This simple implementation uses very dumb "hashset" that just uses
/// vector.

// depends on derivative = "1"
use derivative::Derivative;
use std::rc::{Rc, Weak};
use std::boxed::Box;
use std::marker::PhantomData;
use std::ops::Deref;
use std::hash::{BuildHasher, Hash, Hasher};
use std::collections::hash_map::RandomState;

// I wish it was in std
type HashValue = u64;

#[derive(Clone)]
#[derive(Debug)]
#[derive(Derivative)]
#[derivative(PartialEq)]
pub struct PooledStr<BH: BuildHasher> {
    hash: HashValue,
    value: String,  // just use a ready container
    #[derivative(PartialEq="ignore")]
    _build_hasher: PhantomData<BH>,
}

impl<BH: BuildHasher> PartialEq<str> for PooledStr<BH> {
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
}

impl<BH: BuildHasher> Deref for PooledStr<BH> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.value.deref()
    }
}

pub trait FromHashableSource<V: ?Sized, BH: BuildHasher> {
    fn from_ref(v: &V, hasher: &mut BH::Hasher) -> Self;
    fn from_hashed(hash: HashValue, v: &V) -> Self;
    fn get_hash(&self) -> HashValue;
}

impl<BH: BuildHasher> FromHashableSource<str, BH> for PooledStr<BH> {
    fn from_ref(val: &str, hasher: &mut BH::Hasher) -> PooledStr<BH> {
	val.hash(hasher);
        let hash = hasher.finish();

        PooledStr {
            hash,
            value: String::from(val),
            _build_hasher: PhantomData,
        }
    }

    fn from_hashed(hash: HashValue, val: &str) -> PooledStr<BH> {
        PooledStr {
            hash,
            value: String::from(val),
            _build_hasher: PhantomData,
        }
    }

    fn get_hash(&self) -> HashValue {
	self.hash
    }
}

// Dumb HashSet that has only linear probing
pub struct DumbSet<T, K: ?Sized, BH: BuildHasher> {
    bins: Vec<Weak<T>>,
    builder_hash: BH,
    _phantom: PhantomData<K>,
}

impl<T, K: ?Sized, BH> DumbSet<T, K, BH>
where T: PartialEq<K> + PartialEq + Clone + FromHashableSource<K, BH>,
      K: Hash,
      BH: BuildHasher{
    /// Create new hash; use same builder_hash if you want to
    /// reuse interned values from one pool in another.
    pub fn from_builder(builder_hash: BH) -> Self {
        Self {
            bins: Vec::new(),
            builder_hash,
            _phantom: PhantomData,
        }
    }

    /// Intern key value (e.g. &str), returning new or old interned object
    pub fn intern(&mut self, key: &K) -> Rc<T> {
        let mut hasher = self.builder_hash.build_hasher();
	key.hash(&mut hasher);
        let hash = hasher.finish();

        // Yep, it is very dumb.
        for wc in self.bins.iter() {
            if let Some(rc) = wc.upgrade() {
                if rc.get_hash() == hash && rc.deref() == key {
                    return rc;
                }
            }
        }
        // Not found
        let newval = Rc::new(T::from_hashed(hash, key));
        self.bins.push(Rc::downgrade(&newval));
        newval
    }

    /// Add another interned value, e.g. from another pool.  Or even
    /// reintern string from same pool.
    pub fn implant(&mut self, val: &Rc<T>) {
        for wc in self.bins.iter() {
            if let Some(rc) = wc.upgrade() {
                if rc.deref() == val.deref() {
                    return;
                }
            }
        }
        // Not found.  Very dumb implementation: we do not even look
        // for free week cell.
        self.bins.push(Rc::downgrade(val));
    }

    // So far I have no idea how to do better without Box<dyn ...>
    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item=Rc<T>> + 'a> {
        Box::new(self.bins.iter().filter_map(|wc| wc.upgrade()))
    }
}

pub type Pool<BH> = DumbSet<PooledStr<BH>, str, BH>;


fn main() {
    let random_builder = RandomState::new();
    let mut pool = Pool::from_builder(random_builder);
    let mut interns = Vec::new();

    for w in String::from("Mary had a little lamb, a little lamb, a little lamb.").split_whitespace() {
        interns.push(pool.intern(w));
    }
    for interned in pool.iter() {
        println!("{:?}", interned.deref());
    }
}

/*
Output:

Mary
had
a
little
lamb,
lamb.
*/
