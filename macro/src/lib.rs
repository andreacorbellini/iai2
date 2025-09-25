#![warn(clippy::dbg_macro)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]
#![warn(missing_debug_implementations)]
#![warn(unreachable_pub)]
#![warn(unused_qualifications)]
#![doc(test(attr(deny(warnings))))]

extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenTree};
use quote::quote_spanned;

#[proc_macro_attribute]
pub fn iai(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = proc_macro2::TokenStream::from(item);

    let span = proc_macro2::Span::call_site();

    let function_name = find_name(item.clone());
    let wrapper_function_name = Ident::new(&format!("wrap_{}", function_name), span);
    let const_name = Ident::new(&format!("IAI_FUNC_{}", function_name), span);
    let name_literal = function_name.to_string();

    let output = quote_spanned!(span=>
        #item

        fn #wrapper_function_name() {
            let _ = ::core::hint::black_box(#function_name());
        }

        #[test_case]
        const #const_name : (&'static str, fn()) = (#name_literal, #wrapper_function_name);
    );

    output.into()
}

fn find_name(stream: proc_macro2::TokenStream) -> Ident {
    let mut iter = stream.into_iter();
    for tok in iter.by_ref() {
        if let TokenTree::Ident(ident) = tok
            && ident == "fn"
        {
            break;
        }
    }

    if let Some(TokenTree::Ident(name)) = iter.next() {
        name
    } else {
        panic!("Unable to find function name")
    }
}
