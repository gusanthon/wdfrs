use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::{parse_macro_input, DeriveInput};

mod circuit;
mod export;
mod rtype;

/// Returns a token stream for the root path of the `wdfrs` crate,
/// whether the macro is invoked from inside wdfrs itself (`crate`) or
/// from an external crate (`::wdfrs`).
pub(crate) fn wdfrs_crate() -> TokenStream2 {
    use proc_macro_crate::{crate_name, FoundCrate};
    match crate_name("wdfrs") {
        Ok(FoundCrate::Itself) => quote::quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            quote::quote!(::#ident)
        }
        Err(_) => quote::quote!(::wdfrs),
    }
}

// ── define_rtype / define_root_rtype ────────────────────────────────────────

/// Generate an R-type adaptor struct with `n_ports` total ports.
///
/// Usage: `define_rtype!(MyRType, 5);`
///
/// This creates a struct `MyRType<T, P1, ..., P{n_ports-1}>` with child
/// ports `p1 .. p{n_ports-1}`. One port is designated as the "upward"
/// (parent) connection via `up_port_idx` passed to `::new()`. The
/// N×N scattering matrix is set via `set_scattering_matrix`.
///
/// The calling scope must have `WDF`, `WDFloat`, and `WDFParams` in scope.
#[proc_macro]
pub fn define_rtype(input: TokenStream) -> TokenStream {
    let rtype::RTypeInput { name, n_ports } = parse_macro_input!(input as rtype::RTypeInput);
    rtype::gen_rtype(&name, n_ports).into()
}

/// Generate a root R-type adaptor struct with `n_ports` total ports.
///
/// Usage: `define_root_rtype!(MyRootRType, 4);`
///
/// Unlike `define_rtype`, a root adaptor has **no** parent connection —
/// all N ports connect to children. Call `.compute()` each sample instead
/// of `get_reflected_wave` / `set_incident_wave`.
///
/// The calling scope must have `WDF`, `WDFloat`, `WDFParams`, and
/// `RootWDF` in scope.
#[proc_macro]
pub fn define_root_rtype(input: TokenStream) -> TokenStream {
    let rtype::RTypeInput { name, n_ports } = parse_macro_input!(input as rtype::RTypeInput);
    rtype::gen_root_rtype(&name, n_ports).into()
}

// ── WDFCircuit derive ────────────────────────────────────────────────────────

#[proc_macro_derive(
    WDFCircuit,
    attributes(wdf_root, wdf_v_input, wdf_i_input, wdf_v_output, wdf_i_output)
)]
pub fn derive_wdf_circuit(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    circuit::derive_wdf_circuit_impl(input).into()
}

// ── export_wdf ───────────────────────────────────────────────────────────────

#[proc_macro]
pub fn export_wdf(input: TokenStream) -> TokenStream {
    let export::ExportWdf {
        circuit,
        prefix,
        constructor,
        precision,
        methods,
    } = syn::parse_macro_input!(input as export::ExportWdf);
    export::export_wdf_impl(circuit, prefix, constructor, precision, methods).into()
}
