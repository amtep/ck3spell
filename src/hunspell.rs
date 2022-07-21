use anyhow::{anyhow, Context, Result};
use std::ffi::CString;
use std::fs::File;
use std::os::raw::c_int;
use std::path::{Path, PathBuf};

/// Opaque type representing a Hunhandle in C
#[repr(C)]
struct Hunhandle {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[link(name = "hunspell")]
extern "C" {
    fn Hunspell_create(affpath: *const i8, dpath: *const i8) -> *mut Hunhandle;
    fn Hunspell_destroy(pHunspell: *mut Hunhandle);
    fn Hunspell_spell(pHunspell: *mut Hunhandle, word: *const i8) -> c_int;
    fn Hunspell_add(pHunspell: *mut Hunhandle, word: *const i8) -> c_int;
}

pub struct Hunspell {
    handle: *mut Hunhandle,
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
        let c_dpath =
            Hunspell::_path_helper(path, locale, "dic", "dictionary")?;
        let c_affpath = Hunspell::_path_helper(path, locale, "aff", "affix")?;

        unsafe {
            let handle = Hunspell_create(c_affpath.as_ptr(), c_dpath.as_ptr());
            Ok(Hunspell { handle })
        }
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
