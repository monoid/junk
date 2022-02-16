Rust crate for locking serverl locks at once.  The implementation follows the gcc's libstdc++:
<https://github.com/gcc-mirror/gcc/blob/master/libstdc%2B%2B-v3/include/std/mutex#L635-L682>

The implemented algorithm is the "smart and polite" algorithm from the
<https://howardhinnant.github.io/dining_philosophers.html>.

Please note that this implementation may theoretically livelock under particular
high-contended lock patterns.  Just avoid them :)

## Features
The only feature is the **arrayvec**.
