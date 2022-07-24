use anyhow::{anyhow, Context, Result};
use std::ffi::{CStr, CString};
use std::fs::{read_to_string, File, OpenOptions};
use std::io::Write;
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};
use std::ptr;
use std::rc::Rc;

/// Opaque type representing a Hunhandle in C
#[repr(C)]
struct Hunhandle {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[link(name = "hunspell")]
extern "C" {
    fn Hunspell_create(
        affpath: *const c_char,
        dpath: *const c_char,
    ) -> *mut Hunhandle;
    fn Hunspell_destroy(pHunspell: *mut Hunhandle);
    fn Hunspell_spell(pHunspell: *mut Hunhandle, word: *const c_char) -> c_int;
    fn Hunspell_add(pHunspell: *mut Hunhandle, word: *const c_char) -> c_int;
    fn Hunspell_suggest(
        pHunspell: *mut Hunhandle,
        slst: *const *mut *mut c_char,
        word: *const c_char,
    ) -> c_int;
    fn Hunspell_free_list(
        pHunspell: *mut Hunhandle,
        slst: *const *mut *mut c_char,
        n: c_int,
    );
}

pub struct Hunspell {
    handle: *mut Hunhandle,
    user_dict: Option<PathBuf>,
}

impl Hunspell {
    fn _path_helper(
        path: &Path,
        locale: &str,
        ext: &str,
        errname: &str,
    ) -> Result<CString> {
        let mut p = path.to_path_buf();
        p.push(format!("{}.{}", locale, ext));

        // Hunspell itself won't tell us if opening the dictionary fails,
        // so check it here.
        File::open(&p).with_context(|| {
            format!("Could not open {} file {}", errname, p.display())
        })?;

        // These unwraps won't panic because we have full control over the incoming pathname.
        Ok(CString::new(p.as_os_str().to_str().unwrap()).unwrap())
    }

    pub fn new(path: &Path, locale: &str) -> Result<Hunspell> {
        let c_dpath = Self::_path_helper(path, locale, "dic", "dictionary")?;
        let c_affpath = Self::_path_helper(path, locale, "aff", "affix")?;

        unsafe {
            let handle = Hunspell_create(c_affpath.as_ptr(), c_dpath.as_ptr());
            Ok(Hunspell {
                handle,
                user_dict: None,
            })
        }
    }

    pub fn set_user_dict(&mut self, path: &Path) -> Result<i32> {
        self.user_dict = Some(path.to_path_buf());
        if !path.exists() {
            File::create(path).with_context(|| {
                format!("Could not create {}", path.display())
            })?;
        }
        let dict = read_to_string(path)
            .with_context(|| format!("Could not read {}", path.display()))?;
        let mut added = 0;
        for word in dict.lines() {
            self.add_word(word);
            added += 1;
        }
        Ok(added)
    }

    pub fn spellcheck(&self, word: &str) -> bool {
        let c_word = if let Ok(c_word) = CString::new(word) {
            c_word
        } else {
            return true;
        };
        unsafe {
            let result = Hunspell_spell(self.handle, c_word.as_ptr());
            result != 0
        }
    }

    fn _user_dict_adder(user_dict: &Path, word: &str) -> Result<()> {
        let mut file = OpenOptions::new().append(true).open(user_dict)?;
        file.write_all(word.as_bytes())?;
        file.write_all("\n".as_bytes())?;
        Ok(())
    }

    pub fn add_word_user_dict(&self, word: &str) {
        if let Some(user_dict) = &self.user_dict {
            if let Err(err) = Self::_user_dict_adder(user_dict, word)
                .with_context(|| {
                    format!("Could not append to {}", user_dict.display())
                })
            {
                eprintln!("{:#}", err);
            }
        }
    }

    pub fn add_word(&self, word: &str) {
        let c_word = if let Ok(c_word) = CString::new(word) {
            c_word
        } else {
            return;
        };

        unsafe {
            Hunspell_add(self.handle, c_word.as_ptr());
        }
    }

    pub fn suggestions(&self, word: &str) -> Vec<Rc<String>> {
        let c_word = if let Ok(c_word) = CString::new(word) {
            c_word
        } else {
            return Vec::new();
        };

        unsafe {
            let slstp: *mut *mut c_char = ptr::null_mut();
            let slst = ptr::addr_of!(slstp);
            let len = Hunspell_suggest(self.handle, slst, c_word.as_ptr());
            if len == 0 {
                return Vec::new();
            }

            let ulen = len as usize;
            let mut vec = Vec::new();
            let lst = ptr::slice_from_raw_parts_mut::<*mut c_char>(slstp, ulen);
            let mut i = 0;
            while i < ulen {
                let raw = CStr::from_ptr((*lst)[i]);
                if let Ok(s) = raw.to_str() {
                    vec.push(Rc::new(s.to_string()));
                }
                i += 1;
            }
            Hunspell_free_list(self.handle, slst, len);
            vec
        }
    }

    pub fn find_dictionary<'a>(
        search_path: &[&'a str],
        locale: &str,
    ) -> Result<&'a str> {
        for dir in search_path {
            eprint!("Looking for dictionary in {}", dir);
            let filename = format!("{}.dic", locale);
            let mut p = PathBuf::from(dir);
            p.push(filename);
            if Path::exists(&p) {
                eprintln!(" ...found");
                return Ok(dir);
            }
            eprintln!();
        }
        Err(anyhow!("Dictionary not found"))
    }
}

impl Drop for Hunspell {
    fn drop(&mut self) {
        unsafe {
            Hunspell_destroy(self.handle);
        }
    }
}
