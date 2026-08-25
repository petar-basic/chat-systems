//! `#[db_test]` — a test that gets its own database.
//!
//! Reads like sqlx's own test attribute and replaces it. The difference is
//! underneath: the schema is built once per process into a template, and each
//! test clones it instead of running every migration again.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, ItemFn, LitStr, Token};

struct Args {
    migrations: Option<LitStr>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self { migrations: None });
        }
        let key: syn::Ident = input.parse()?;
        if key != "migrations" {
            return Err(syn::Error::new(
                key.span(),
                "expected `migrations = \"...\"`",
            ));
        }
        input.parse::<Token![=]>()?;
        let migrations: LitStr = input.parse()?;
        Ok(Self {
            migrations: Some(migrations),
        })
    }
}

#[proc_macro_attribute]
pub fn db_test(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as Args);
    let function = parse_macro_input!(item as ItemFn);

    let migrations = args
        .migrations
        .unwrap_or_else(|| LitStr::new("./migrations", proc_macro2::Span::call_site()));

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
        ..
    } = function;

    let (pool_arg, pool_type) = match sig.inputs.first() {
        Some(syn::FnArg::Typed(arg)) => (arg.pat.clone(), arg.ty.clone()),
        _ => {
            return syn::Error::new_spanned(
                &sig,
                "a #[db_test] takes exactly one argument: the pool for its own database",
            )
            .to_compile_error()
            .into()
        }
    };

    let name = &sig.ident;
    let output = &sig.output;

    quote! {
        #(#attrs)*
        #[::tokio::test]
        #vis async fn #name() #output {
            // Resolved against the crate root so the path reads the same from
            // anywhere cargo happens to run the binary from.
            let __migrations = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(#migrations);
            let __db = ::shared_common::testing::TestDb::create(
                __migrations.to_str().expect("a utf-8 migrations path"),
            )
            .await;
            let #pool_arg: #pool_type = __db.pool();

            #block

            __db.cleanup().await;
        }
    }
    .into()
}
