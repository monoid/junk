#![doc = include_str!("../README.md")]

use std::{
    sync::{Mutex, MutexGuard, PoisonError, TryLockError},
    thread::yield_now,
};

/**
 * Lock several locks at once.  Algorithms is taken from the gcc's libstdc++:
 * lock the first lock and try_lock the next one; if some one fails,
 * repeat starting with locking the failed one and try_locking others in cyclic manner.
 *
 * However, the order of result items does match the order of the input mutexes.
 */
pub fn lock_many_vec<'a, T>(
    mutices: &[&'a Mutex<T>],
) -> Result<Vec<MutexGuard<'a, T>>, PoisonError<MutexGuard<'a, T>>> {
    let len = mutices.len();
    let mut res = Vec::with_capacity(len);
    let mut start = 0;

    'retry: loop {
        res.push(mutices[start].lock()?);
        for n in 1..len {
            let idx = (start + n) % len;
            match mutices[idx].try_lock() {
                Ok(guard) => res.push(guard),
                Err(TryLockError::Poisoned(e)) => return Err(e),
                Err(TryLockError::WouldBlock) => {
                    res.clear();
                    start = idx;
                    yield_now();
                    continue 'retry;
                }
            }
        }
        res.rotate_right(start);
        return Ok(res);
    }
}

// TODO replace both implementations with macro.
#[cfg(feature = "arrayvec")]
/**
 * Lock several locks at once.  Algorithms is taken from the gcc's libstdc++:
 * lock the first lock and try_lock the next one; if some one fails,
 * repeat starting with locking the failed one and try_locking others in cyclic manner.
 *
 * However, the order of result items does match the order of the input mutexes.
 */
pub fn lock_many_arrayvec<'a, T, const N: usize>(
    mutices: &[&'a Mutex<T>],
) -> Result<arrayvec::ArrayVec<MutexGuard<'a, T>, N>, PoisonError<MutexGuard<'a, T>>> {
    let len = mutices.len();
    let mut res = arrayvec::ArrayVec::new();
    let mut start = 0;

    'retry: loop {
        res.push(mutices[start].lock()?);
        for n in 1..len {
            let idx = (start + n) % len;
            match mutices[idx].try_lock() {
                Ok(guard) => res.push(guard),
                Err(TryLockError::Poisoned(e)) => return Err(e),
                Err(TryLockError::WouldBlock) => {
                    res.clear();
                    start = idx;
                    yield_now();
                    continue 'retry;
                }
            }
        }
        res.rotate_right(start);
        return Ok(res);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ops::Deref,
        sync::{Arc, Mutex},
        thread::{sleep, spawn},
        time::Duration,
    };

    #[cfg(feature = "arrayvec")]
    use crate::lock_many_arrayvec;
    use crate::lock_many_vec;

    #[test]
    fn it_works() {
        let mut1 = Mutex::new(42);
        let mut2 = Mutex::new(43);
        assert_eq!(
            42 + 43,
            lock_many_vec(&[&mut1, &mut2])
                .unwrap()
                .into_iter()
                .map(|g| *g)
                .sum::<i32>()
        );
    }

    #[test]
    fn test_ordering() {
        let mutices = Arc::new((0..4).map(Mutex::new).collect::<Vec<_>>());
        let mutices2 = mutices.clone();
        let holder = spawn(move || {
            let g = mutices2[2].lock().unwrap();
            sleep(Duration::from_millis(300));
            std::mem::drop(g);
        });
        sleep(Duration::from_millis(200));
        let mutices_ref = mutices.iter().collect::<Vec<&Mutex<_>>>();
        let g = lock_many_vec(&mutices_ref).unwrap();
        let g_val = g.iter().map(Deref::deref).collect::<Vec<_>>();
        assert_eq!(&g_val, &[&0, &1, &2, &3]);
        holder.join().unwrap();
    }

    #[cfg(feature = "arrayvec")]
    #[test]
    fn it_works_arrayvec() {
        let mut1 = Mutex::new(42);
        let mut2 = Mutex::new(43);
        assert_eq!(
            42 + 43,
            lock_many_arrayvec::<_, 2>(&[&mut1, &mut2])
                .unwrap()
                .into_iter()
                .map(|g| *g)
                .sum::<i32>()
        );
    }

    #[test]
    #[cfg(feature = "arrayvec")]
    fn test_ordering_arrayvec() {
        let mutices = Arc::new((0..4).map(Mutex::new).collect::<Vec<_>>());
        let mutices2 = mutices.clone();
        let holder = spawn(move || {
            let g = mutices2[2].lock().unwrap();
            sleep(Duration::from_millis(200));
            std::mem::drop(g);
        });
        sleep(Duration::from_millis(100));
        let mutices_ref = mutices.iter().collect::<Vec<&Mutex<_>>>();
        let g = lock_many_arrayvec::<_, 4>(&mutices_ref).unwrap();
        let g_val = g.iter().map(Deref::deref).collect::<Vec<_>>();
        assert_eq!(&g_val, &[&0, &1, &2, &3]);
        holder.join().unwrap();
    }

    #[test]
    fn test_parallel() {
        let mut1 = Arc::new(Mutex::new((0, 0)));
        let mut2 = Arc::new(Mutex::new((0, 0)));
        const REPEAT: i32 = 1000000;
        let complete = Arc::new(Mutex::new(0));
        let mut cnt = 0;

        let mut1a = mut1.clone();
        let completea = complete.clone();
        let tha = std::thread::spawn(move || {
            for _ in 0..REPEAT {
                mut1a.lock().unwrap().0 += 1;
            }
            let mut compl_guard = completea.lock().unwrap();
            *compl_guard += 1;
        });
        let mut2b = mut2.clone();
        let completeb = complete.clone();
        let thb = std::thread::spawn(move || {
            for _ in 0..REPEAT {
                mut2b.lock().unwrap().0 += 1;
            }
            let mut compl_guard = completeb.lock().unwrap();
            *compl_guard += 1;
        });

        loop {
            let complete_val = *complete.lock().unwrap();
            cnt += 1;
            let locks = lock_many_vec(&[mut1.deref(), mut2.deref()]).unwrap();
            for mut g in locks.into_iter() {
                g.1 += g.0;
                g.0 = 0;
            }

            if complete_val == 2 {
                break;
            }
        }

        tha.join().unwrap();
        thb.join().unwrap();

        {
            let g1 = mut1.lock().unwrap();
            assert_eq!(g1.0, 0);
            assert_eq!(g1.1, REPEAT);
        }

        let g2 = mut2.lock().unwrap();
        assert_eq!(g2.0, 0);
        assert_eq!(g2.1, REPEAT);

        // Well, may fail on particular scheduling...
        assert!(cnt > 200, "{}", cnt);
    }
}
