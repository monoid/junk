/*! Primitive Mark'n'Sweep garbage collection runtime.  Please note
 * this is not a GC for Rust, but GC in Rust.
 *
 * Each object on stack or heap has an usize tag at offset zero; by
 * this tag, the runtime will know object type and thus pointer
 * offsets.  The tag also designates a moved object if it is odd (thus
 * this code requires that allocated buffers are even-byte aligned; on
 * some architectures some precautions should be taken.
 *
 * Each stack frame has internal pointer to the parent frame.  Stack
 * frames are not moved, and they are detected by its non-zero
 * TypeDesc::parent value.
 */

mod mem;
mod stack;

use std::{cmp::{max, min}, fmt::Debug};
use std::{ffi::c_void, ptr::null_mut};
use thiserror::Error;

use stack::find_range_in_mem_file;

/**
Type description: list of local offsets for pointers.
 */
/* TODO: should we limit object size (and field offsets) to u16, u32
 * or just usize?
 * TODO: tell if pointer is a vector in the field.
*/
#[derive(Debug)]
pub struct TypeDesc<T: AsRef<[usize]> + Debug> {
    /// Object used to be moved.  Includes tag size as well.
    pub size: usize,
    /// Offsets of all pointers fields.
    // TODO smallvec or slice to an external buffer.
    pub offsets: T,
    /// Stack frame parent field offset; zero if type is not a stack frame.
    // TODO: non-zero type
    pub parent: usize,
}

pub type FinalTypeDesc = TypeDesc<Box<[usize]>>;

pub enum ObjectInfo<'a> {
    Forward(*mut usize),
    // We presume that TypeDescs have static lifetime relatively to
    // the runtime.
    Object(&'a FinalTypeDesc),
}

#[derive(Error, Debug)]
pub enum AllocError {
    #[error("Failed to allocate {} words", .words)]
    OutOufMemory { words: usize },
}

pub struct Arena<M: mem::Mem> {
    base: *mut usize,
    end: *mut usize,
    current: *mut usize,

    stack_range: (usize, usize),
    menace: std::marker::PhantomData<M>,
}

impl<M: mem::Mem> Drop for Arena<M> {
    fn drop(&mut self) {
        unsafe {
            M::from_raw(self.base, self.end.offset_from(self.base) as _);
        }
    }
}

impl<M: mem::Mem> Arena<M> {
    pub fn from_memory(memory: M) -> Result<Self, std::io::Error> {
        let (base, len) = mem::Mem::to_raw(memory);

        unsafe {
            match Self::from_range(base, len) {
                Ok(a) => Ok(a),
                Err(e) => {
                    M::from_raw(base, len); // free memory
                    Err(e)
                }
            }
        }
    }

    unsafe fn from_range(base: *mut usize, len: usize) -> Result<Self, std::io::Error> {
        let end = base.add(len);
        let stack_range = find_range_in_mem_file((&end) as *const _ as usize)?.unwrap();
        Ok(Self {
            base,
            current: end,
            end,
            stack_range,
            menace: std::marker::PhantomData,
        })
    }

    /// Returns an unitialized object pointer.  Only tag field is initialized.
    #[inline]
    pub unsafe fn alloc(&mut self, type_desc: &FinalTypeDesc) -> Result<*mut usize, AllocError> {
        let len = type_desc.size;
        let new_addr = self.current.sub(len);
        if self.base <= new_addr {
            self.current = new_addr;
            new_addr.write(type_desc as *const _ as usize);
            Ok(new_addr)
        } else {
            Err(AllocError::OutOufMemory { words: len })
        }
    }

    pub fn size(&self) -> usize {
        unsafe { self.end.offset_from(self.base) as _ }
    }

    pub fn free(&self) -> usize {
        unsafe { self.current.offset_from(self.base) as _ }
    }
}

pub struct Gc<M: mem::Mem> {
    arena: Arena<M>,
    max_size: usize,
}

impl<M: mem::Mem> Gc<M> {
    pub fn new(size: usize, max_size: usize) -> Self {
        Gc {
            arena: Arena::from_memory(M::new(size)).unwrap(),
            max_size,
        }
    }

    #[inline]
    pub unsafe fn alloc(
        &mut self,
        type_desc: &FinalTypeDesc,
        stack: *mut c_void,
    ) -> Result<*mut c_void, AllocError> {
        match self.arena.alloc(type_desc) {
            Ok(addr) => Ok(addr as _),
            Err(_) => self.alloc_after_gc(type_desc, stack).map(|x| x as _),
        }
    }

    unsafe fn alloc_after_gc(
        &mut self,
        type_desc: &FinalTypeDesc,
        stack: *mut c_void,
    ) -> Result<*mut usize, AllocError> {
        self.gc(type_desc.size, stack)?;
        self.arena.alloc(type_desc)
    }

    unsafe fn gc(&mut self, extra_size: usize, stack: *mut c_void) -> Result<(), AllocError> {
        // TODO re-use.
        let mut ptr_stack: Vec<*mut *mut usize> = vec![];
        let new_size = self.arena.size() + max(self.arena.size(), extra_size);
        let maxed_new_size = min(new_size, self.max_size);
        let mut new_arena = Arena::from_memory(M::new(maxed_new_size))
            .expect("Failed to allocate arena; TODO better handling");
        // TODO try_into()
        let mut stack = stack as *mut usize;

        while stack != null_mut() {
            stack = Self::run_gc_step(stack, &mut ptr_stack, &mut new_arena);
        }

        std::mem::swap(&mut self.arena, &mut new_arena);
        Ok(())
    }

    unsafe fn run_gc_step(
        stack: *mut usize,
        ptr_stack: &mut Vec<*mut *mut usize>,
        target: &mut Arena<M>,
    ) -> *mut usize {
        let next = match read_object_info(stack) {
            ObjectInfo::Forward(_) => {
                unreachable!()
            }
            ObjectInfo::Object(type_desc) => {
                push_fields(stack, type_desc, ptr_stack);
                if type_desc.parent == 0 {
                    panic!("Found non-stack stack frame at {:x}", stack as usize);
                } else {
                    stack.add(type_desc.parent).read() as _
                }
            }
        };
        while let Some(field) = ptr_stack.pop() {
            move_object(field, target, ptr_stack);
        }
        next
    }
}

unsafe fn push_fields(
    object: *mut usize,
    type_desc: &FinalTypeDesc,
    ptr_stack: &mut Vec<*mut *mut usize>,
) {
    for field_offset in type_desc.offsets.iter().cloned() {
        let field_ptr = object.add(field_offset) as *mut *mut usize;

        if !field_ptr.read().is_null() {
            ptr_stack.push(field_ptr);
        }
    }
}

unsafe fn move_object<M: mem::Mem>(
    field: *mut *mut usize,
    target: &mut Arena<M>,
    ptr_stack: &mut Vec<*mut *mut usize>,
) {
    let obj_ptr = field.read();

    field.write(match read_object_info(obj_ptr) {
        ObjectInfo::Forward(new_loc) => {
            // Already forwarded
            new_loc
        }
        ObjectInfo::Object(type_desc) => {
            // Hard lifting.

            // It is safe to unwrap, as new arena is not smaller than
            // old one.
            let new_loc = target.alloc(type_desc).unwrap();
            // use byte copy, as it may be undefined behavior on some exotic
            // architectures.  Or not?
            new_loc.copy_from(obj_ptr, type_desc.size);
            // Address with tag.
            obj_ptr.write(new_loc as usize | 1);
            push_fields(new_loc, type_desc, ptr_stack);
            new_loc
        }
    });
}

//#[inline]
pub unsafe fn read_object_info<'a>(ptr: *const usize) -> ObjectInfo<'a> {
    let tag = ptr.read();

    match (tag & 1) != 0 {
        false => {
            match (tag as *const FinalTypeDesc).as_ref() {
                Some(desc) => ObjectInfo::Object(desc),
                // TODO panic is quite lengthy and shouldn't be inlined.
                // Should we move it to separate function?
                None => panic!("Null type descriptor for object at {:?}", ptr),
            }
        }
        true => {
            let forward_ptr = (tag - 1) as *const *mut usize;
            ObjectInfo::Forward(forward_ptr.read())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_drop() {
        // We should run it with valgrind.
        // But at least it checks that stack info is detected.
        Arena::from_memory(vec![0; 32].into_boxed_slice()).unwrap();
    }

    #[test]
    fn test_arena_alloc_success() {
        let type_info = vec![
            FinalTypeDesc {
                size: 1,
                offsets: vec![].into_boxed_slice(),
                parent: 0,
            },
            FinalTypeDesc {
                size: 2,
                offsets: vec![1].into_boxed_slice(),
                parent: 0,
            },
        ];
        let mut a = Arena::from_memory(vec![0; 32].into_boxed_slice()).unwrap();

        unsafe {
            a.alloc(&type_info[0]).unwrap();
            a.alloc(&type_info[1]).unwrap();
            assert_eq!(
                a.end.offset_from(a.current) as usize,
                type_info[0].size + type_info[1].size
            );
        }
    }

    #[test]
    fn test_gc_alloc_success() {
        let type_info = vec![
            FinalTypeDesc {
                size: 1,
                offsets: vec![].into_boxed_slice(),
                parent: 0,
            },
            FinalTypeDesc {
                size: 2,
                offsets: vec![1].into_boxed_slice(),
                parent: 0,
            },
            FinalTypeDesc {
                size: 3,
                offsets: vec![2].into_boxed_slice(),
                parent: 1,
            },
        ];
        let mut gc = Gc::<Box<[usize]>>::new(32, 1024);

        eprintln!("type_info base: {:x}", &type_info[0] as *const _ as usize);
        let mut stack = [&type_info[2] as *const _ as usize, 0usize, 0usize];
        let stack_ptr = stack.as_mut_ptr() as _;

        unsafe {
            gc.alloc(&type_info[0], stack_ptr).unwrap();
            gc.alloc(&type_info[1], stack_ptr).unwrap();
            assert_eq!(
                gc.arena.end.offset_from(gc.arena.current) as usize,
                type_info[0].size + type_info[1].size
            );
        }
    }

    #[test]
    fn test_gc_alloc_many() {
        let type_info = vec![
            FinalTypeDesc {
                size: 1,
                offsets: vec![].into_boxed_slice(),
                parent: 0,
            },
            FinalTypeDesc {
                size: 2,
                offsets: vec![1].into_boxed_slice(),
                parent: 0,
            },
            FinalTypeDesc {
                size: 3,
                offsets: vec![2].into_boxed_slice(),
                parent: 1,
            },
        ];
        let mut gc = Gc::<Box<[usize]>>::new(1, 1024);

        let mut stack = [&type_info[2] as *const _ as usize, 0usize, 0usize];
        let stack_ptr = stack.as_mut_ptr() as _;

        let count = 100;
        unsafe {
            stack[2] = gc.alloc(&type_info[0], stack_ptr).unwrap() as _;
            for _ in 0..count {
                let new_obj = gc.alloc(&type_info[1], stack_ptr).unwrap() as *mut usize;
                new_obj.add(1).write(stack[2]);
                stack[2] = new_obj as _;
            }
            assert_eq!(
                gc.arena.end.offset_from(gc.arena.current) as usize,
                type_info[0].size + count * type_info[1].size
            );
        }
    }

    #[test]
    fn test_gc_alloc_overflow() {
        let type_info = vec![
            FinalTypeDesc {
                size: 1,
                offsets: vec![].into_boxed_slice(),
                parent: 0,
            },
            FinalTypeDesc {
                size: 2048,
                offsets: vec![1].into_boxed_slice(),
                parent: 0,
            },
            FinalTypeDesc {
                size: 3,
                offsets: vec![2].into_boxed_slice(),
                parent: 1,
            },
        ];
        let mut gc = Gc::<Box<[usize]>>::new(1, 1024);
        let mut stack = [&type_info[2] as *const _ as usize, 0usize, 0usize];
        let stack_ptr = stack.as_mut_ptr() as _;

        unsafe {
            stack[2] = gc.alloc(&type_info[0], stack_ptr).unwrap() as _;
            assert!(gc.alloc(&type_info[1], stack_ptr).is_err());
        }
    }
}
