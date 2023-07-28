use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

use quote::{quote, ToTokens};
use syn::{
	Data, DataEnum, DataStruct, DeriveInput, Expr, ExprLit, Fields, Lit, Meta, MetaList,
	MetaNameValue,
};

/// Parse struct fields into an iterator over the
/// doc comments of fields in the order of definition.
fn parse_doc_comments_from_fields(fields: &Fields) -> impl Iterator<Item = String> + '_ {
	fields.iter().map(|field| {
		let mut doc_comments = vec![];

		// Every individual doc comment is an attr.
		field.attrs.iter().for_each(|attr| {
			if let Meta::NameValue(MetaNameValue { path, value, .. }) = &attr.meta {
				path.segments.iter().for_each(|segment| {
					if segment.ident == "doc" {
						if let Expr::Lit(ExprLit {
							lit: Lit::Str(lit_str),
							..
						}) = value
						{
							let mut raw_token = lit_str.token().to_string();
							if let Some(stripped) = raw_token.strip_prefix('\"') {
								raw_token = stripped.to_string();
							}
							if let Some(stripped) = raw_token.strip_suffix('\"') {
								raw_token = stripped.to_string();
							}
							// Collect every line of doc-comment.
							doc_comments.push(raw_token.trim().to_string());
						}
					}
				});
			}
		});

		if doc_comments.is_empty() {
			return "No doc comment found".to_string();
		}
		doc_comments.join(" ")
	})
}

/// Parse fields for the widgets to generate from the `#[control]` field attributes.
fn parse_widgets_from_fields(fields: &Fields) -> impl Iterator<Item = TokenStream2> + '_ {
	fields.iter().flat_map(|field| {
		let name = field.ident.clone().unwrap();
		field.attrs.iter().filter_map(move |attr| {
			if let Meta::List(MetaList { path, tokens, .. }) = &attr.meta {
				if path.into_token_stream().to_string() == "control" {
					let mut token_iter = tokens.clone().into_iter();
					if let Some(proc_macro2::TokenTree::Ident(ident)) = token_iter.next() {
						if ident == "slider" {
							let proc_macro2::TokenTree::Group(group) = token_iter
								.next()
								.expect("slider to be provided a InclusiveRange prop")
							else {
								panic!("slider expects an InclusiveRange prop.");
							};
							let stream = group.stream();
							return Some(quote!(
									::bevy_egui::egui::Slider::new(&mut self.#name, #stream)
							));
						} else if ident == "textbox" {
							return Some(quote!(
									::bevy_egui::egui::TextEdit::singleline(&mut self.#name).hint_text("")
							));
						} else if ident == "bool" {
							return Some(quote! {
									::bevy_egui::egui::Checkbox::without_text(&mut self.#name)
							});
						}
						return None;
					}
				}
			}
			None
		})
	})
}

/// Expand the parsed struct into a [bevy_egui::egui::Grid] of three columns
/// where the first column is the struct field name, the second column
/// is the interactive form control, and the third field is the description
/// of the field extracted from the doc comment.
pub fn expand(input: DeriveInput) -> TokenStream {
	match &input.data {
		Data::Struct(DataStruct { fields, .. }) => {
			let struct_name = &input.ident;
			let field_docs = parse_doc_comments_from_fields(fields);
			let field_widgets = parse_widgets_from_fields(fields);

			let expanded = quote! {
					impl #struct_name {
							pub fn ui(&mut self, ui: &mut ::bevy_egui::egui::Ui) -> ::bevy_egui::egui::Response {
								ui.with_layout(::bevy_egui::egui::Layout::top_down(::bevy_egui::egui::Align::Min), |ui| {
											#(
													{
														ui.horizontal_wrapped(|ui| {
															ui.add(#field_widgets);
															ui.label(#field_docs);
														});
													}
											)*
								})
									.response
							}
					}
			};
			expanded.into()
		}
		Data::Enum(DataEnum { .. }) => {
			let enum_name = &input.ident;

			let expanded = quote! {
				impl #enum_name {
					pub fn ui(&mut self, ui: &mut ::bevy_egui::egui::Ui) -> ::bevy_egui::egui::Response {
						ui.with_layout(
							::bevy_egui::egui::Layout::top_down(::bevy_egui::egui::Align::Min),
							|ui| {
								for variant in <#enum_name as ::strum::IntoEnumIterator>::iter() {
									ui.selectable_value(self, variant, format!("{}", variant));
								}
							},
						).response
					}
				}
			};

			expanded.into()
		}
		_ => panic!("expected a struct or enum"),
	}
}
