use libc::{c_char, c_int, size_t};
use std::ffi::{CStr, CString};
use std::ptr;

use crate::compile;
use crate::types::TokenVector;

/// C-compatible function to compile text using Morphlex
/// 
/// # Arguments
/// * `input` - Pointer to null-terminated input string
/// * `input_len` - Length of input string in bytes
/// * `tokens_ptr` - Pointer to store token array (caller must allocate or pass NULL)
/// * `token_count` - Pointer to store number of tokens
/// * `lemmas_ptr` - Pointer to store lemma array (caller must allocate or pass NULL)
/// * `lemma_count` - Pointer to store number of lemmas
/// 
/// # Returns
/// * 0 on success
/// * Negative error code on failure
/// 
/// # Safety
/// Caller must ensure that:
/// * `input` is a valid pointer to `input_len` bytes
/// * If `tokens_ptr` is not NULL, it points to allocated memory for at least `*token_count` TokenVector elements
/// * If `lemmas_ptr` is not NULL, it points to allocated memory for at least `*lemma_count` *mut c_char elements
/// * All allocated memory will be freed by the caller
#[unsafe(no_mangle)]
pub unsafe extern "C" fn morphlex_compile(
    input: *const c_char,
    input_len: size_t,
    tokens_ptr: *mut TokenVector,
    token_count: *mut size_t,
    lemmas_ptr: *mut *mut c_char,
    lemma_count: *mut size_t,
) -> c_int {
    // Safety: Check that input is not null
    if input.is_null() {
        return -1;
    }
    
    // Safety: Create string slice from input
    let input_slice = unsafe {
        std::slice::from_raw_parts(input as *const u8, input_len as usize)
    };
    
    // Safety: Convert to UTF-8 string
    let input_str = match std::str::from_utf8(input_slice) {
        Ok(s) => s,
        Err(_) => return -2, // Invalid UTF-8
    };
    
    // Call the Morphlex compile function
    let result = match compile(input_str) {
        Ok((lemmas, vectors)) => {
            // Set output counts
            unsafe {
                *token_count = vectors.len() as size_t;
                *lemma_count = lemmas.len() as size_t;
            }
            
            // If caller provided output buffers, copy data to them
            if !tokens_ptr.is_null() && vectors.len() > 0 {
                // Copy tokens
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        vectors.as_ptr(),
                        tokens_ptr,
                        vectors.len()
                    );
                }
            }
            
            if !lemmas_ptr.is_null() && lemmas.len() > 0 {
                // Copy lemmas (each needs to be allocated as CString)
                unsafe {
                    for (i, lemma) in lemmas.iter().enumerate() {
                        // Convert String to CString and allocate memory
                        let c_string = CString::new(lemma.as_str()).unwrap();
                        let ptr = c_string.into_raw();
                        
                        // Store pointer in the output array
                        *lemmas_ptr.add(i) = ptr;
                    }
                }
            }
            
            Ok(())
        }
        Err(e) => {
            eprintln!("Morphlex compile error: {}", e);
            Err(())
        }
    };
    
    match result {
        Ok(_) => 0,
        Err(_) => -3,
    }
}

/// Free memory allocated by morphlex_compile for lemmas
/// 
/// # Arguments
/// * `ptr` - Pointer to array of lemma pointers
/// * `count` - Number of lemma pointers in the array
/// 
/// # Safety
/// Caller must ensure that:
/// * `ptr` points to an array of at least `count` *mut c_char elements
/// * Each element in the array is either NULL or a pointer allocated by morphlex_compile
#[unsafe(no_mangle)]
pub unsafe extern "C" fn morphlex_free_lemmas(ptr: *mut *mut c_char, count: size_t) {
    if ptr.is_null() || count == 0 {
        return;
    }
    
    unsafe {
        for i in 0..count as usize {
            let lemma_ptr = *ptr.add(i);
            if !lemma_ptr.is_null() {
                // Safety: This pointer was allocated by morphlex_compile as a CString
                let _ = CString::from_raw(lemma_ptr);
            }
        }
        
        // Free the array itself
        unsafe {
            libc::free(ptr as *mut libc::c_void);
        }
    }
}

/// Free memory allocated by morphlex_compile for tokens
/// 
/// # Arguments
/// * `ptr` - Pointer to token array
/// 
/// # Safety
/// Caller must ensure that:
/// * `ptr` is either NULL or a pointer allocated by morphlex_compile
#[unsafe(no_mangle)]
pub unsafe extern "C" fn morphlex_free_tokens(ptr: *mut TokenVector) {
    if !ptr.is_null() {
        unsafe {
            libc::free(ptr as *mut libc::c_void);
        }
    }
}