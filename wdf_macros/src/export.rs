use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, ExprCall, ExprClosure, ExprPath, Ident, Pat, Token, Type,
};

pub(crate) struct ExportWdf {
    pub circuit: Option<Type>,
    pub prefix: Option<Ident>,
    pub constructor: ExprClosure,
    pub precision: Type,
    pub methods: Vec<Ident>,
}

impl Parse for ExportWdf {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut circuit: Option<Type> = None;
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
    circuit: Option<Type>,
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
    let circuit_ty = match circuit {
        Some(t) => t,
        None => {
            return syn::Error::new(proc_macro2::Span::call_site(), "missing `circuit`")
                .to_compile_error()
        }
    };

    // `circuit` is written the way the type is spelled in Rust, with one
    // addition: `_` stands for the root the constructor picked. A circuit
    // generic over its root is `DiodeClipper<f32, _>`; one that names its root
    // itself — owning several trees, say — is `MyCircuit<f32>`; one with no
    // generics at all is a bare `Clip`.
    let (base_name, self_ty, root_is_generic) = match circuit_shape(&circuit_ty) {
        Ok(shape) => shape,
        Err(e) => return e.to_compile_error(),
    };

    let trait_name = format_ident!("{}{}FfiTrait", base_name, prec_upper);

    // The `_` the shape replaced with `R` is the only generic the impl binds.
    let generics = if root_is_generic {
        quote! { <R: #rwdf::core::wdf::RootWDF<#precision>> }
    } else {
        quote! {}
    };
    let circuit_type = format_ident!("__{}_{}__Circuit", prefix, prec_str);
    let build_fn = format_ident!("__{}_{}_build", prefix, prec_str);

    // The alias only has to be opaque when a `_` left the root generic: there
    // is no way to spell the root the constructor picked. A type written out in
    // full is already nameable, so the alias is an ordinary one and the
    // consumer needs no `type_alias_impl_trait` on account of this macro.
    let (circuit_alias, define_opaque) = if root_is_generic {
        (
            quote! { type #circuit_type = impl #trait_name; },
            quote! { #[define_opaque(#circuit_type)] },
        )
    } else {
        (quote! { type #circuit_type = #self_ty; }, quote! {})
    };

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
                fn __check_method #generics (s: &mut #self_ty) {
                    let _: fn(&mut #self_ty, #precision) = |s, v| s.#m(v);
                }
            };
        }
    });

    quote! {
        #( #method_checks )*

        pub trait #trait_name: #rwdf::core::circuit::WDFCircuit<#precision> {
            #( #method_trait_fns )*
        }

        impl #generics #trait_name for #self_ty {
            #( #method_impls )*
        }

        #[allow(non_camel_case_types)]
        #circuit_alias

        #[allow(non_snake_case)]
        #[allow(clippy::redundant_closure)]
        #define_opaque
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
            #rwdf::core::circuit::WDFCircuit::process(f, input)
        }

        #[no_mangle]
        pub unsafe extern "C" fn #process_block_fn(
            ptr: *mut (),
            input: *const #precision,
            output: *mut #precision,
            len: usize,
        ) {
            let f = &mut *(ptr as *mut #circuit_type);
            // Read and write through the raw pointers directly. Hosts routinely
            // pass the same buffer as input and output, and a `&[T]` plus a
            // `&mut [T]` over the same block alias. Reading each sample before
            // writing its slot keeps in-place processing correct.
            for i in 0..len {
                let x = *input.add(i);
                *output.add(i) = #rwdf::core::circuit::WDFCircuit::process(f, x);
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

/// Reads the `circuit` type into the name the FFI trait is derived from, the
/// self type to write the impl for, and whether the root was left generic.
///
/// A `_` among the type arguments marks the root the constructor picked, which
/// nothing can name; it is replaced by `R`, the one generic the impl binds.
/// A type with no `_` is nameable as written and used verbatim.
fn circuit_shape(ty: &Type) -> syn::Result<(Ident, Type, bool)> {
    let Type::Path(tp) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "`circuit` must be a path type, e.g. `DiodeClipper<f32, _>` or `MyCircuit<f32>`",
        ));
    };

    let mut tp = tp.clone();
    let segment = tp
        .path
        .segments
        .last_mut()
        .ok_or_else(|| syn::Error::new_spanned(ty, "`circuit` needs a type name"))?;
    let base_name = segment.ident.clone();

    let mut holes = 0usize;
    if let syn::PathArguments::AngleBracketed(ab) = &mut segment.arguments {
        for arg in ab.args.iter_mut() {
            if let syn::GenericArgument::Type(t @ Type::Infer(_)) = arg {
                holes += 1;
                *t = syn::parse_quote!(R);
            }
        }
    }

    match holes {
        0 => Ok((base_name, Type::Path(tp), false)),
        1 => Ok((base_name, Type::Path(tp), true)),
        _ => Err(syn::Error::new_spanned(
            ty,
            "`circuit` takes at most one `_`, for the root the constructor picks",
        )),
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
