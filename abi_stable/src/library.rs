/*!
Traits and types related to loading an abi_stable dynamic library,
as well as functions/modules within.
*/

use std::{
    fmt::{self, Display},
    io,
    marker::PhantomData,
    mem,
    path::{Path,PathBuf},
    sync::atomic,
};

#[allow(unused_imports)]
use core_extensions::prelude::*;

use libloading::{
    Library as LibLoadingLibrary,
    Symbol as LLSymbol,
};

use abi_stable_derive_lib::{
    mangled_root_module_loader_name,
};



use crate::{
    abi_stability::{
        AbiInfoWrapper,
        AbiInfo,
        stable_abi_trait::SharedStableAbi,
    },
    globals::{self,Globals},
    marker_type::ErasedObject,
    version::{ParseVersionError, VersionNumber, VersionStrings},
    utils::{transmute_reference},
    std_types::{RVec,RBoxError,StaticStr},
};


mod lib_header;
mod root_mod_trait;
mod raw_library;


pub use self::{
    lib_header::{AbiHeader,LibHeader},
    root_mod_trait::{
        RootModule,
        lib_header_from_raw_library,
        lib_header_from_path,
        RootModuleConsts,
        ErasedRootModuleConsts,
    },
    raw_library::RawLibrary,
};


///////////////////////////////////////////////////////////////////////////////


/// What naming convention to expect when loading a library from a directory.
#[derive(Debug,Copy,Clone,PartialEq,Eq,Ord,PartialOrd,Hash)]
pub enum LibrarySuffix{
    /// Loads a dynamic library at `<folder>/<base_name>.extension`
    NoSuffix,
    
    /// Loads a dynamic library at `<folder>/<base_name>-<pointer_size>.<extension>`
    Suffix,
}


//////////////////////////////////////////////////////////////////////

/// The path a library is loaded from.
#[derive(Debug,Copy,Clone,PartialEq,Eq,Ord,PartialOrd,Hash)]
pub enum LibraryPath<'a>{
    FullPath(&'a Path),
    Directory(&'a Path),
}

//////////////////////////////////////////////////////////////////////


/// All the possible errors that could happen when loading a library,
/// or a module.
#[derive(Debug)]
pub enum LibraryError {
    /// When a library can't be loaded, because it doesn't exist.
    OpenError{
        path:PathBuf,
        io:io::Error,
    },
    /// When a function/static does not exist.
    GetSymbolError{
        library:PathBuf,
        /// The name of the function/static.Does not have to be utf-8.
        symbol:Vec<u8>,
        io:io::Error,
    },
    /// The version string could not be parsed into a version number.
    ParseVersionError(ParseVersionError),
    /// The version numbers of the library was incompatible.
    IncompatibleVersionNumber {
        library_name: &'static str,
        expected_version: VersionNumber,
        actual_version: VersionNumber,
    },
    /// The abi is incompatible.
    /// The error is opaque,since the error always comes from the main binary
    /// (dynamic libraries can be loaded from other dynamic libraries),
    /// and no approach for extensible enums is settled on yet.
    AbiInstability(RBoxError),
    /// The type used to check that this is a compatible abi_stable
    /// is not the same.
    InvalidAbiHeader(AbiHeader),
    /// There could have been 0 or more errors in the function.
    Many(RVec<Self>),
}

impl From<ParseVersionError> for LibraryError {
    fn from(v: ParseVersionError) -> LibraryError {
        LibraryError::ParseVersionError(v)
    }
}

impl Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\n")?;
        match self {
            LibraryError::OpenError{path,io} => writeln!(
                f,
                "Could not open library at:\n\t{}\nbecause:\n\t{}",
                path.display(),io
            ),
            LibraryError::GetSymbolError{library,symbol,io} => writeln!(
                f,
                "Could load symbol:\n\t{}\nin library:\n\t{}\nbecause:\n\t{}",
                String::from_utf8_lossy(symbol),
                library.display(),
                io
            ),
            LibraryError::ParseVersionError(x) => fmt::Display::fmt(x, f),
            LibraryError::IncompatibleVersionNumber {
                library_name,
                expected_version,
                actual_version,
            } => writeln!(
                f,
                "\n'{}' library version mismatch:\nuser:{}\nlibrary:{}",
                library_name, expected_version, actual_version,
            ),
            LibraryError::AbiInstability(x) => fmt::Display::fmt(x, f),
            LibraryError::InvalidAbiHeader(found) => write!(
                f,
                "The abi of the library was:\n{:#?}\n\
                 When this library expected:\n{:#?}",
                found, AbiHeader::VALUE,
            ),
            LibraryError::Many(list)=>{
                for e in list {
                    Display::fmt(e,f)?;
                }
                Ok(())
            }
        }?;
        f.write_str("\n")?;
        Ok(())
    }
}

impl ::std::error::Error for LibraryError {}
