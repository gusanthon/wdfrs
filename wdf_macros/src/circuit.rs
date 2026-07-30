use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Ident, Type};

pub(crate) fn derive_wdf_circuit_impl(input: DeriveInput) -> TokenStream2 {
    let rwdf = crate::wdfrs_crate();
    let name = &input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let root_field = find_field_with_attr(&input.data, "wdf_root")
        .expect("Must designate one field with #[wdf_root]");
    let root_type = find_field_ty_with_attr(&input.data, "wdf_root")
        .expect("Must designate one field with #[wdf_root]");

    let voltage_input = find_field_with_attr(&input.data, "wdf_v_input");
    let current_input = find_field_with_attr(&input.data, "wdf_i_input");
    let voltage_outputs = find_fields_with_attr(&input.data, "wdf_v_output");
    let current_outputs = find_fields_with_attr(&input.data, "wdf_i_output");
    let handle_inits = generate_handle_inits(&input.data, &root_field);
    let ft = get_float_type(&input.generics, &input.data);

    let field_inits = if let Data::Struct(ds) = &input.data {
        ds.fields
            .iter()
            .map(|f| {
                let name_ident = f.ident.as_ref().expect("WDFCircuit requires named fields");
                let field_ty = &f.ty;

                if f.attrs.iter().any(|a| a.path().is_ident("wdf_root")) {
                    return quote! { #name_ident: ::core::pin::Pin::new(::std::boxed::Box::new(root)) };
                }

                if is_handle_type(field_ty) {
                    return quote! {
                        #name_ident: unsafe { <#field_ty>::__internal_uninit() }
                    };
                }

                quote! { #name_ident: Default::default() }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let default_trait_impl = {
        let (in_f, set_fn) = if let Some(f) = &voltage_input {
            (Some(f), quote! { set_voltage })
        } else if let Some(f) = &current_input {
            (Some(f), quote! { set_current })
        } else {
            (None, quote! { set_voltage })
        };

        if let Some(in_f) = in_f {
            let output_sum = if voltage_outputs.is_empty() && current_outputs.is_empty() {
                quote! { #ft::zero() }
            } else {
                let v_outs = voltage_outputs
                    .iter()
                    .map(|f| quote! { self.#f.wave_to_voltage() });
                let c_outs = current_outputs
                    .iter()
                    .map(|f| quote! { self.#f.wave_to_current() });
                let all_outputs: Vec<_> = v_outs.chain(c_outs).collect();
                quote! { #( #all_outputs )+* }
            };

            quote! {
                impl #impl_generics #rwdf::core::circuit::WDFCircuit<#ft> for #name #ty_generics #where_clause {
                    #[inline(always)]
                    fn process(&mut self, sample: #ft) -> #ft {
                        self.#in_f.#set_fn(sample);
                        self.#root_field.compute();
                        #output_sum
                    }

                    fn reset(&mut self) {
                        self.#root_field.reset_tree();
                    }
                }
            }
        } else {
            quote! {}
        }
    };

    let inner_root_type = extract_inner_type_from_pin_box(&root_type).unwrap_or(root_type.clone());

    quote! {
        #default_trait_impl

        #[allow(unexpected_cfgs)]
        impl #impl_generics #name #ty_generics #where_clause {
            pub fn init_handles(&mut self) {
                unsafe {
                    #( #handle_inits )*
                }
            }

            /// Creates a new circuit from the root component.
            /// It automatically pins and boxes the root, initializes all handles, and defaults other fields.
            pub fn from_root(root: #inner_root_type) -> Box<Self> {
                let mut circuit = Box::new(Self {
                    #( #field_inits, )*
                });
                circuit.init_handles();
                circuit
            }

            pub fn recalc_impedance(&mut self) {
                self.#root_field.calc_impedance();
            }

            pub fn set_sample_rate(&mut self, fs: #ft) {
                self.#root_field.set_tree_sample_rate(fs);
            }

            pub fn print_spqr_tree(&self) {
                let s = format!("{} SPQR Tree", stringify!(#name));
                let underline = "-".repeat(s.len());
                println!("\n\n{}\n{}", s, underline);
                self.#root_field.print_full_tree();
            }

            pub fn for_each_wdf(&self, f: &mut dyn FnMut(&dyn #rwdf::core::wdf::WDF<#ft>)) {
                self.#root_field.for_each_tree(f);
            }

        }
    }
}

fn generate_handle_inits(data: &Data, root_name: &Ident) -> Vec<TokenStream2> {
    let mut inits = Vec::new();
    if let Data::Struct(ds) = data {
        for field in &ds.fields {
            let field_ident = field.ident.as_ref().expect("Fields must be named");

            if is_handle_type(&field.ty) {
                let id_str = field_ident.to_string();
                inits.push(quote! {
                    self.#field_ident = self.#root_name.get_handle_for_id(#id_str);
                });
            }
        }
    }
    inits
}

fn is_handle_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident.to_string().contains("Handle");
        }
    }
    false
}

pub(crate) fn get_float_type(generics: &syn::Generics, data: &Data) -> TokenStream2 {
    for param in &generics.params {
        if let syn::GenericParam::Type(type_param) = param {
            let ident = &type_param.ident;

            for bound in &type_param.bounds {
                if let syn::TypeParamBound::Trait(tb) = bound {
                    if let Some(last_seg) = tb.path.segments.last() {
                        let seg_ident = last_seg.ident.to_string();

                        // Case 1: The generic IS the float type (e.g., T: WDFloat)
                        if seg_ident == "WDFloat" {
                            return quote! { #ident };
                        }
                        // Case 2: The generic is the Root, and the float type is inside the angle brackets (e.g., R: RootWDF<f64>)
                        else if seg_ident == "RootWDF" || seg_ident == "WDF" {
                            if let syn::PathArguments::AngleBracketed(ab) = &last_seg.arguments {
                                if let Some(syn::GenericArgument::Type(inner_ty)) = ab.args.first()
                                {
                                    return quote! { #inner_ty };
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Case 3: no generic float — a precision-hardcoded circuit. Every handle
    // names its float as its first type argument (`ResVSHandle<f32>`,
    // `WDFHandle<f32, W>`), so read it off the first handle field.
    if let Some(ty) = float_from_handle_field(data) {
        return quote! { #ty };
    }

    // No handle to read and no generic: default to f64, which is correct for a
    // circuit whose components are themselves f64.
    quote! { f64 }
}

/// The float type of the first handle field, if any — its first type argument.
fn float_from_handle_field(data: &Data) -> Option<Type> {
    let Data::Struct(ds) = data else { return None };
    for field in &ds.fields {
        if !is_handle_type(&field.ty) {
            continue;
        }
        if let Type::Path(tp) = &field.ty {
            if let Some(seg) = tp.path.segments.last() {
                if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(ty)) = ab.args.first() {
                        return Some(ty.clone());
                    }
                }
            }
        }
    }
    None
}

fn find_field_with_attr(data: &Data, attr_name: &str) -> Option<Ident> {
    find_fields_with_attr(data, attr_name).into_iter().next()
}

fn find_field_ty_with_attr(data: &Data, attr_name: &str) -> Option<Type> {
    if let Data::Struct(ds) = data {
        for field in &ds.fields {
            if field.attrs.iter().any(|a| a.path().is_ident(attr_name)) {
                return Some(field.ty.clone());
            }
        }
    }
    None
}

fn find_fields_with_attr(data: &Data, attr_name: &str) -> Vec<Ident> {
    let mut fields = Vec::new();
    if let Data::Struct(ds) = data {
        for field in &ds.fields {
            if field.attrs.iter().any(|a| a.path().is_ident(attr_name)) {
                if let Some(ident) = &field.ident {
                    fields.push(ident.clone());
                }
            }
        }
    }
    fields
}

fn extract_inner_type_from_pin_box(ty: &Type) -> Option<Type> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            // WDFRoot<R> is a type alias for Pin<Box<R>> — extract R directly.
            if seg.ident == "WDFRoot" {
                if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = ab.args.first() {
                        return Some(inner_ty.clone());
                    }
                }
            }
            if seg.ident == "Pin" {
                if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(Type::Path(inner_tp))) = ab.args.first()
                    {
                        if let Some(inner_seg) = inner_tp.path.segments.last() {
                            if inner_seg.ident == "Box" {
                                if let syn::PathArguments::AngleBracketed(inner_ab) =
                                    &inner_seg.arguments
                                {
                                    if let Some(syn::GenericArgument::Type(final_ty)) =
                                        inner_ab.args.first()
                                    {
                                        return Some(final_ty.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
