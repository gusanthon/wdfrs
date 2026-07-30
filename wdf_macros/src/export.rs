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

    // `circuit` takes either a bare name (`DiodeClipper`), which is assumed to
    // be the usual `<T, R>` circuit shape and gets an impl generic over the
    // root; or a type with its arguments already applied (`MyCircuit<f32>`),
    // for a circuit that names its root concretely — one owning several trees,
    // say, with no single root to leave generic.
    //
    // Which of the two is meant is read off the spelling, not the type: a
    // circuit generic over `T` alone still has to be written `MyCircuit<f32>`.
    let Some((base_name, root_is_generic)) = circuit_shape(&circuit_ty) else {
        return syn::Error::new_spanned(
            &circuit_ty,
            "`circuit` must be a path type, e.g. `DiodeClipper` or `MyCircuit<f32>`",
        )
        .to_compile_error();
    };

    let trait_name = format_ident!("{}{}FfiTrait", base_name, prec_upper);

    // Where the impl and the method signature checks get their generics.
    let (impl_generics, self_ty, check_generics, check_ty, check_val) = if root_is_generic {
        (
            quote! { <R: #rwdf::core::wdf::RootWDF<#precision>> },
            quote! { #circuit_ty<#precision, R> },
            quote! { <T: #rwdf::core::wdf::WDFloat, R: #rwdf::core::wdf::RootWDF<T>> },
            quote! { #circuit_ty<T, R> },
            quote! { T },
        )
    } else {
        (
            quote! {},
            quote! { #circuit_ty },
            quote! {},
            quote! { #circuit_ty },
            quote! { #precision },
        )
    };
    let circuit_type = format_ident!("__{}_{}__Circuit", prefix, prec_str);
    let build_fn = format_ident!("__{}_{}_build", prefix, prec_str);

    // The alias only has to be opaque when the root is still generic: there is
    // no way to spell `#circuit_ty<#precision, R>` for the `R` the constructor
    // picked. With the arguments applied the type is already nameable, so the
    // alias is an ordinary one and the consumer needs no `type_alias_impl_trait`
    // on account of this macro.
    let (circuit_alias, define_opaque) = if root_is_generic {
        (
            quote! { type #circuit_type = impl #trait_name; },
            quote! { #[define_opaque(#circuit_type)] },
        )
    } else {
        (quote! { type #circuit_type = #circuit_ty; }, quote! {})
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
                fn __check_method #check_generics (s: &mut #check_ty) {
                    let _: fn(&mut #check_ty, #check_val) = |s, v| s.#m(v);
                }
            };
        }
    });

    quote! {
        #( #method_checks )*

        pub trait #trait_name: #rwdf::core::circuit::WDFCircuit<#precision> {
            #( #method_trait_fns )*
        }

        impl #impl_generics #trait_name for #self_ty {
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

/// Splits the `circuit` type into its base name and whether the root is still
/// generic — that is, whether the caller left the type's arguments off.
///
/// Returns `None` for anything that is not a path type.
fn circuit_shape(ty: &Type) -> Option<(Ident, bool)> {
    let Type::Path(tp) = ty else { return None };
    let segment = tp.path.segments.last()?;
    let root_is_generic = matches!(segment.arguments, syn::PathArguments::None);

    Some((segment.ident.clone(), root_is_generic))
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
