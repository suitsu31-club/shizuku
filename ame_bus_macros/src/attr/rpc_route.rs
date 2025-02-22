use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::Variant;

/// Args of `#[rpc_route(service = "service_name")]` attribute.
struct RpcEndpointRouteArgs {
    pub service: Ident,
}

impl Parse for RpcEndpointRouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let _: Ident = content.parse()?;
        let _: syn::Token![=] = content.parse()?;
        let service: syn::LitStr = content.parse()?;
        Ok(RpcEndpointRouteArgs {
            service: Ident::new(&service.value(), service.span()),
        })
    }
}

/// Args of `#[rpc_endpoint(request = "request_name")]` attribute.
struct RpcEndpointArgs {
    pub request: Ident,
}

impl Parse for RpcEndpointArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let _: Ident = content.parse()?;
        let _: syn::Token![=] = content.parse()?;
        let request: syn::LitStr = content.parse()?;
        Ok(RpcEndpointArgs {
            request: Ident::new(&request.value(), request.span()),
        })
    }
}

struct RouteTableEnumInfo {
    pub service: Ident,
    pub routes: Vec<(Variant, RpcEndpointArgs)>,
    pub enum_ident: Ident,
}

impl RouteTableEnumInfo {
    pub fn generate_subject_mappings(&self) -> Vec<TokenStream> {
        let enum_ident = self.enum_ident.clone();
        self.routes.iter()
            .map(|(variant, RpcEndpointArgs {request})| {
                quote! {
                    (#request::subject(), #enum_ident::#variant)
                }
            })
            .collect()
    }
    pub fn route_match_arms(&self) -> Vec<TokenStream> {
        let enum_ident = self.enum_ident.clone();
        self.routes.iter()
            .map(|(variant, RpcEndpointArgs {request})| {
                quote! {
                    Some(#enum_ident::#variant) => #request::process_request(
                        service.as_ref(),
                        #request::parse_request(&request)?,
                    )
                    .await
                    .map(|response| response.to_bytes())
                }
            })
            .collect()
    }
    pub fn streaming_endpoints(&self) -> Vec<TokenStream> {
        self.routes.iter()
            .map(|(_, RpcEndpointArgs {request})| {
                quote! {
                    service.endpoint(#request::ENDPOINT_NAME).await?;
                }
            })
            .collect()
    }
}