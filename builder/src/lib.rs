use std::rc::Rc;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, token::Token, Attribute, Data, DeriveInput, Field, Fields, GenericArgument,
    Ident, Lit, LitStr, Meta, PathArguments, Type, TypePath,
};

struct FieldData<'a> {
    ident: &'a Ident,
    attr: Option<Ident>,
    inner_ty: &'a TypePath,
    is_option: bool,
}

impl<'a> FieldData<'a> {
    pub fn new(
        ident: &'a Ident,
        attr: Option<Ident>,
        inner_ty: &'a TypePath,
        is_option: bool,
    ) -> Self {
        Self {
            ident,
            attr,
            inner_ty,
            is_option,
        }
    }
}

struct FieldDataList<'a>(Vec<FieldData<'a>>);

impl<'a> FieldDataList<'a> {
    fn get_ident(&self) -> Vec<&'a Ident> {
        self.0.iter().map(|data| data.ident).collect()
    }

    fn get_attrs(&self) -> Vec<&Option<Ident>> {
        self.0.iter().map(|data| &data.attr).collect()
    }

    fn get_inner_ty(&self) -> Vec<&'a TypePath> {
        self.0.iter().map(|data| data.inner_ty).collect()
    }

    fn get_is_option(&self) -> Vec<bool> {
        self.0.iter().map(|data| data.is_option).collect()
    }
}

impl<'a> FromIterator<FieldData<'a>> for FieldDataList<'a> {
    fn from_iter<T: IntoIterator<Item = FieldData<'a>>>(iter: T) -> Self {
        FieldDataList(iter.into_iter().collect())
    }
}

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    if let Data::Struct(data_struct) = input.data {
        if let Fields::Named(fields_named) = data_struct.fields {
            let data = parse_field_data(fields_named.named.iter().collect());
            let builder_name = format_ident!("{}Builder", name);

            let build_impl = build_impl(&name, &builder_name, &data);
            let build_builder_struct = build_builder_struct(&builder_name, &data);
            let build_builder_impl = build_builder_impl(&name, &builder_name, &data);

            let tokens = quote! {
                #build_impl
                #build_builder_struct
                #build_builder_impl
            }
            .into();
            eprintln!("TOKENS: {}", tokens);
            return tokens;
        }
    }
    TokenStream::new()
}

fn build_impl(
    name: &Ident,
    builder_name: &Ident,
    data: &FieldDataList,
) -> proc_macro2::TokenStream {
    let field_names = data.get_ident();
    let attrs = data.get_attrs();
    let init_builder_vals = init_builder_vals(&field_names, &attrs);
    quote! {
        impl #name {
            pub fn builder() -> #builder_name {
                #builder_name {
                    #(#init_builder_vals)*
                }
            }
        }
    }
    .into()
}

fn init_builder_vals(
    field_names: &Vec<&Ident>,
    attrs: &Vec<&Option<Ident>>,
) -> Vec<proc_macro2::TokenStream> {
    let mut return_values = Vec::new();
    for i in 0..field_names.len() {
        let name = field_names[i];
        let attr = attrs[i];

        return_values.push(match attr {
            Some(_) => quote! {
                #name: std::option::Option::Some(Vec::new()),
            },
            None => quote! {
                #name: std::option::Option::None,
            },
        });
    }
    return_values
}

fn build_builder_struct(builder_name: &Ident, data: &FieldDataList) -> proc_macro2::TokenStream {
    let field_names = data.get_ident();
    let inners = data.get_inner_ty();
    quote! {
        pub struct #builder_name {
            #(#field_names: std::option::Option<#inners>,)*
        }
    }
    .into()
}

fn build_builder_impl(
    name: &Ident,
    builder_name: &Ident,
    data: &FieldDataList,
) -> proc_macro2::TokenStream {
    let field_names = data.get_ident();
    let field_attrs = data.get_attrs();
    let inners = data.get_inner_ty();
    let is_options = data.get_is_option();

    let builder_funcs = builder_funcs(&field_names, &inners, &field_attrs);
    let build_method = build_method(name, &field_names, &is_options);
    quote! {
        impl #builder_name {
            #builder_funcs
            #build_method
        }
    }
    .into()
}

fn builder_funcs(
    field_names: &Vec<&Ident>,
    inners: &Vec<&TypePath>,
    attrs: &Vec<&Option<Ident>>,
) -> proc_macro2::TokenStream {
    let builder_funcs = init_builder_funcs(field_names, inners, attrs);
    quote! {
        #(#builder_funcs)*
    }
    .into()
}

fn init_builder_funcs(
    field_names: &Vec<&Ident>,
    inners: &Vec<&TypePath>,
    attrs: &Vec<&Option<Ident>>,
) -> Vec<proc_macro2::TokenStream> {
    let mut return_values = Vec::new();
    for i in 0..field_names.len() {
        let name = field_names[i];
        let inner = inners[i];
        let attr = attrs[i];

        return_values.push(match attr {
            Some(ident) => {
                // Assume at this point we have a vec
                let (_, inner) = inner_type(inner, "Vec");
                quote! {
                    fn #ident(&mut self, #ident: #inner) -> &mut Self {
                        self.#name.as_mut().unwrap().push(#ident);
                        self
                    }
                }
            }
            None => quote! {
                fn #name(&mut self, #name: #inner) -> &mut Self {
                    self.#name = std::option::Option::Some(#name);
                    self
                }
            },
        });
    }
    return_values
}

fn build_method(
    name: &Ident,
    field_names: &Vec<&Ident>,
    is_options: &Vec<bool>,
) -> proc_macro2::TokenStream {
    let check_set = check_set(field_names, is_options);
    let field_return_values = field_return_values(field_names, is_options);
    quote! {
        pub fn build(&mut self) -> Result<#name, Box<dyn std::error::Error>> {
            #check_set

            Ok(#name {
                #(#field_return_values)*
            })
        }
    }
    .into()
}

fn check_set(field_names: &Vec<&Ident>, is_options: &Vec<bool>) -> proc_macro2::TokenStream {
    quote! {
        if #(self.#field_names.is_none() && !#is_options ||)* false {
            Err("Field is not set")?
        }
    }
}

fn field_return_values(
    field_names: &Vec<&Ident>,
    is_options: &Vec<bool>,
) -> Vec<proc_macro2::TokenStream> {
    let mut return_values = Vec::new();
    for i in 0..field_names.len() {
        let name = field_names[i];
        let is_option = is_options[i];

        return_values.push(build_field_value(name, is_option));
    }
    return_values
}

fn parse_field_data(fields: Vec<&Field>) -> FieldDataList<'_> {
    let fields = fields
        .iter()
        .map(|field| {
            if let Type::Path(ty) = &field.ty {
                let (is_option, inner_ty) = inner_type(&ty, "Option");
                let attr = match field.attrs.first() {
                    Some(attr) => match parse_builder_attribute(attr) {
                        Ok(ident) => Some(ident),
                        Err(_) => None,
                    },
                    None => None,
                };
                FieldData::new(field.ident.as_ref().unwrap(), attr, inner_ty, is_option)
            } else {
                panic!(
                    "Problem with parsing field {}",
                    field.ident.as_ref().unwrap()
                );
            }
        })
        .collect();

    fields
}

fn build_field_value(field_name: &Ident, is_option: bool) -> proc_macro2::TokenStream {
    match is_option {
        true => quote! {
            #field_name: self.#field_name.clone(),
        }
        .into(),
        false => quote! {
            #field_name: self.#field_name.clone().unwrap(),
        }
        .into(),
    }
}

fn inner_type<'a>(ty: &'a TypePath, outer: &str) -> (bool, &'a TypePath) {
    let segment = ty.path.segments.first().unwrap();
    if segment.ident.to_string() == outer {
        if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
            if let Some(GenericArgument::Type(Type::Path(path))) = arguments.args.first() {
                return (true, path);
            }
        }
        return (true, ty);
    }
    (false, ty)
}

fn parse_builder_attribute(attr: &Attribute) -> Result<Ident, syn::Error> {
    let mut each: Ident = format_ident!("cheese");
    if attr.path().is_ident("builder") {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("each") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                each = format_ident!("{}", s.value());
                Ok(())
            } else {
                Err(meta.error("Invalid attribute; expected #[builder(each=\'...\')]"))
            }
        })?;
    }
    Ok(each)
}
