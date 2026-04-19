use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, ExprCall, ExprClosure, ExprPath, Ident, Pat, Token, Type,
};

pub(crate) struct ExportWdf {
    pub circuit: Option<Ident>,
    pub prefix: Option<Ident>,
    pub constructor: ExprClosure,
    pub precision: Type,
    pub methods: Vec<Ident>,
}

impl Parse for ExportWdf {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut circuit: Option<Ident> = None;
        let mut prefix: Option<Ident> = None;
        let mut constructor: Option<ExprClosure> = None;
        let mut precision: Option<Type> = None;
        let mut methods: Vec<Ident> = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "prefix" => prefix = Some(input.parse()?),
                "circuit" => circuit = Some(input.parse()?),
                "constructor" => constructor = Some(input.parse()?),
                "precision" => precision = Some(input.parse()?),
                "methods" => {
                    let content;
                    syn::bracketed!(content in input);
                    methods = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?
                        .into_iter()
                        .collect();
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown key `{other}` (expected one of: `circuit`, `prefix`, `constructor`, `precision`, `methods`)",
                        ),
                    ))
                }
            }
            let _ = input.parse::<Token![,]>();
        }

        Ok(Self {
            prefix,
            circuit,
            constructor: constructor.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "missing `constructor`")
            })?,
            precision: precision.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "missing `precision`")
            })?,
            methods,
        })
    }
}

pub(crate) fn export_wdf_impl(
    circuit: Option<Ident>,
    prefix: Option<Ident>,
    constructor: ExprClosure,
    precision: Type,
    methods: Vec<Ident>,
) -> TokenStream2 {
    let rwdf = crate::wdfrs_crate();
    let args = extract_args(&constructor);
    let prec_str = quote!(#precision).to_string();
    let prec_upper = prec_str.to_uppercase();

    let fn_name_str = constructor_name(&constructor)
        .map(|n: Ident| n.to_string())
        .unwrap_or_default();

    let prefix = prefix.unwrap_or_else(|| format_ident!("{}", derive_prefix(&fn_name_str)));
    let struct_name = match circuit {
        Some(n) => n,
        None => {
            return syn::Error::new(proc_macro2::Span::call_site(), "missing `circuit`")
                .to_compile_error()
        }
    };

    let trait_name = format_ident!("{}{}FfiTrait", struct_name, prec_upper);
    let circuit_type = format_ident!("__{}_{}__Circuit", prefix, prec_str);
    let build_fn = format_ident!("__{}_{}_build", prefix, prec_str);
    let new_fn = format_ident!("{}_{}_new", prefix, prec_str);
    let process_fn = format_ident!("{}_{}_process", prefix, prec_str);
    let process_block_fn = format_ident!("{}_{}_process_block", prefix, prec_str);
    let free_fn = format_ident!("{}_{}_free", prefix, prec_str);

    let method_trait_fns = methods.iter().map(|m| {
        quote! {
            fn #m(&mut self, val: #precision);
        }
    });
    let method_impls = methods.iter().map(|m| {
        quote! {
            fn #m(&mut self, val: #precision) { self.#m(val); }
        }
    });
    let method_extern_fns = methods.iter().map(|m| {
        let fn_name = format_ident!("{}_{}_{}", prefix, prec_str, m);
        quote! {
            #[no_mangle]
            pub unsafe extern "C" fn #fn_name(ptr: *mut (), val: #precision) {
                let f = &mut *(ptr as *mut #circuit_type);
                #trait_name::#m(f, val);
            }
        }
    });
    let method_checks = methods.iter().map(|m| {
        quote! {
            const _: () = {
                fn __check_method<T: #rwdf::core::wdf::WDFloat, R: #rwdf::core::wdf::RootWDF<T>>(
                    s: &mut #struct_name<T, R>
                ) {
                    let _: fn(&mut #struct_name<T, R>, T) = |s, v| s.#m(v);
                }
            };
        }
    });

    quote! {
        #( #method_checks )*

        pub trait #trait_name: #rwdf::core::circuit::WDFCircuit<#precision> {
            #( #method_trait_fns )*
        }

        impl<R: #rwdf::core::wdf::RootWDF<#precision>> #trait_name
            for #struct_name<#precision, R>
        {
            #( #method_impls )*
        }

        #[allow(non_camel_case_types)]
        type #circuit_type = impl #trait_name;

        #[allow(non_snake_case)]
        #[allow(clippy::redundant_closure)]
        #[define_opaque(#circuit_type)]
        fn #build_fn(#( #args: #precision, )*) -> Box<#circuit_type> {
            (#constructor)(#( #args, )*)
        }

        #[no_mangle]
        pub extern "C" fn #new_fn(#( #args: #precision, )*) -> *mut () {
            let circuit = #build_fn(#( #args, )*);
            ::std::boxed::Box::into_raw(circuit) as *mut ()
        }

        #[no_mangle]
        pub unsafe extern "C" fn #process_fn(ptr: *mut (), input: #precision) -> #precision {
            let f = &mut *(ptr as *mut #circuit_type);
            f.process(input)
        }

        #[no_mangle]
        pub unsafe extern "C" fn #process_block_fn(
            ptr: *mut (),
            input: *const #precision,
            output: *mut #precision,
            len: usize,
        ) {
            let f = &mut *(ptr as *mut #circuit_type);
            let input = ::core::slice::from_raw_parts(input, len);
            let output = ::core::slice::from_raw_parts_mut(output, len);
            for i in 0..len {
                output[i] = f.process(input[i]);
            }
        }

        #( #method_extern_fns )*

        #[no_mangle]
        pub unsafe extern "C" fn #free_fn(ptr: *mut ()) {
            if !ptr.is_null() {
                ::core::mem::drop(::std::boxed::Box::from_raw(ptr as *mut #circuit_type));
            }
        }
    }
}

fn extract_args(closure: &ExprClosure) -> Vec<Ident> {
    closure
        .inputs
        .iter()
        .filter_map(|pat| {
            if let Pat::Ident(p) = pat {
                Some(p.ident.clone())
            } else {
                None
            }
        })
        .collect()
}

fn constructor_name(closure: &ExprClosure) -> Option<Ident> {
    if let Expr::Call(ExprCall { func, .. }) = closure.body.as_ref() {
        if let Expr::Path(ExprPath { path, .. }) = func.as_ref() {
            return path.get_ident().cloned();
        }
    }
    None
}

pub(crate) fn derive_prefix(name: &str) -> String {
    let stripped = name.strip_prefix("build_").unwrap_or(name);
    stripped
        .split('_')
        .filter_map(|w| w.chars().next())
        .collect()
}
