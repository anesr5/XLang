use crop::Rope;
use dashmap::DashMap;
use log::debug;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use x::{AnalysisResult, SymbolKind, analyze_source, build_hover_at_offset, format_type_name, HoverContext};

const KEYWORDS: &[&str] = &[
    "module", "import", "struct", "const", "return", "if", "else", "true", "false",
];

const TYPES: &[&str] = &["i32", "bool", "void", "str"];

#[derive(Debug)]
struct Backend {
    client: Client,
    document_map: DashMap<String, Rope>,
    analysis_map: DashMap<String, AnalysisResult>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "xlang-language-server".to_owned(),
                version: Some("0.1.0".to_owned()),
            }),
            offset_encoding: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: TextDocumentRegistrationOptions {
                                document_selector: Some(vec![DocumentFilter {
                                    language: Some("xlang".to_owned()),
                                    scheme: Some("file".to_owned()),
                                    pattern: None,
                                }]),
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                legend: SemanticTokensLegend {
                                    token_types: vec![
                                        SemanticTokenType::FUNCTION,
                                        SemanticTokenType::VARIABLE,
                                        SemanticTokenType::PARAMETER,
                                        SemanticTokenType::STRUCT,
                                        SemanticTokenType::PROPERTY,
                                        SemanticTokenType::TYPE,
                                        SemanticTokenType::KEYWORD,
                                    ],
                                    token_modifiers: vec![SemanticTokenModifier::READONLY],
                                },
                                range: Some(true),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                                ..Default::default()
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        debug!("xlang language server initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(TextDocumentChange {
            uri: params.text_document.uri.to_string(),
            text: &params.text_document.text,
        })
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.on_change(TextDocumentChange {
            text: &params.content_changes[0].text,
            uri: params.text_document.uri.to_string(),
        })
        .await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {}

    async fn did_close(&self, _: DidCloseTextDocumentParams) {}

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(self.get_definition(params))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        Ok(self.get_references(
            uri,
            position,
            params.context.include_declaration,
        ))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri.to_string();
        Ok(self.build_semantic_tokens(&uri).map(|data| {
            SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data,
            })
        }))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri.to_string();
        Ok(self
            .build_semantic_tokens_range(&uri, params.range)
            .map(|data| {
                SemanticTokensRangeResult::Tokens(SemanticTokens {
                    result_id: None,
                    data,
                })
            }))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;
        Ok(self.get_hover(&uri, position))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        Ok(self
            .get_completion(&uri)
            .map(CompletionResponse::Array))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        Ok(self.get_rename_edit(uri, position, params.new_name))
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        analysis_map: DashMap::new(),
        document_map: DashMap::new(),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}

impl Backend {
    async fn on_change(&self, item: TextDocumentChange<'_>) {
        let rope = Rope::from(item.text);
        let analysis = analyze_source(item.text);

        let diagnostics = analysis
            .diagnostics
            .iter()
            .filter_map(|diag| diagnostic_to_lsp(diag, &rope))
            .collect::<Vec<_>>();

        let uri =
            Url::parse(&item.uri).unwrap_or_else(|_| Url::from_directory_path(&item.uri).unwrap());
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;

        self.analysis_map.insert(item.uri.clone(), analysis);
        self.document_map.insert(item.uri.clone(), rope);
    }

    fn get_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let rope = self.document_map.get(&uri)?;
        let analysis = self.analysis_map.get(&uri)?;
        let offset = position_to_offset(params.text_document_position_params.position, &rope)?;

        if let Some(symbol_id) = analysis.index.symbol_at_offset(offset) {
            let symbol = analysis.index.symbol(symbol_id)?;
            return location_from_span(
                &params.text_document_position_params.text_document.uri,
                symbol.span,
                &rope,
            )
            .map(GotoDefinitionResponse::Scalar);
        }

        let reference = analysis.index.reference_at_offset(offset)?;
        let symbol = analysis.index.symbol(reference.symbol_id)?;
        location_from_span(
            &params.text_document_position_params.text_document.uri,
            symbol.span,
            &rope,
        )
        .map(GotoDefinitionResponse::Scalar)
    }

    fn get_references(
        &self,
        uri: String,
        position: Position,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        let rope = self.document_map.get(&uri)?;
        let analysis = self.analysis_map.get(&uri)?;
        let offset = position_to_offset(position, &rope)?;
        let parsed_uri =
            Url::parse(&uri).unwrap_or_else(|_| Url::from_directory_path(&uri).unwrap());

        let symbol_id = analysis
            .index
            .symbol_at_offset(offset)
            .or_else(|| {
                analysis
                    .index
                    .reference_at_offset(offset)
                    .map(|reference| reference.symbol_id)
            })?;

        let mut locations = Vec::new();
        if include_declaration {
            let symbol = analysis.index.symbol(symbol_id)?;
            if let Some(location) = location_from_span(&parsed_uri, symbol.span, &rope) {
                locations.push(location);
            }
        }

        for reference in analysis.index.references_to(symbol_id) {
            if let Some(location) = location_from_span(&parsed_uri, reference.span, &rope) {
                locations.push(location);
            }
        }

        Some(locations)
    }

    fn get_rename_edit(
        &self,
        uri: String,
        position: Position,
        new_name: String,
    ) -> Option<WorkspaceEdit> {
        let references = self.get_references(uri.clone(), position, true)?;
        let parsed_uri =
            Url::parse(&uri).unwrap_or_else(|_| Url::from_directory_path(&uri).unwrap());
        let edits = references
            .into_iter()
            .map(|location| TextEdit {
                range: location.range,
                new_text: new_name.clone(),
            })
            .collect::<Vec<_>>();
        let mut changes = std::collections::HashMap::new();
        changes.insert(parsed_uri, edits);
        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }

    fn get_hover(&self, uri: &str, position: Position) -> Option<Hover> {
        let rope = self.document_map.get(uri)?;
        let analysis = self.analysis_map.get(uri)?;
        let offset = position_to_offset(position, &rope)?;
        let source = rope.to_string();

        let markdown = build_hover_at_offset(&HoverContext {
            source: &source,
            program: analysis.program.as_ref(),
            index: &analysis.index,
            offset,
        })?;

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        })
    }

    fn get_completion(&self, uri: &str) -> Option<Vec<CompletionItem>> {
        let analysis = self.analysis_map.get(uri)?;
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for keyword in KEYWORDS {
            if seen.insert(keyword.to_string()) {
                items.push(CompletionItem {
                    label: (*keyword).to_owned(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }
        }

        for ty in TYPES {
            if seen.insert(ty.to_string()) {
                items.push(CompletionItem {
                    label: (*ty).to_owned(),
                    kind: Some(CompletionItemKind::TYPE_PARAMETER),
                    detail: Some("builtin type".to_owned()),
                    ..Default::default()
                });
            }
        }

        for symbol in analysis.index.completion_symbols() {
            if !seen.insert(symbol.name.clone()) {
                continue;
            }
            let (kind, detail) = match symbol.kind {
                SymbolKind::Function => {
                    let ret = symbol
                        .ty
                        .as_ref()
                        .map(format_type_name)
                        .unwrap_or("void");
                    (
                        CompletionItemKind::FUNCTION,
                        Some(format!("{ret} {}()", symbol.name)),
                    )
                }
                SymbolKind::Parameter => (
                    CompletionItemKind::VARIABLE,
                    symbol
                        .ty
                        .as_ref()
                        .map(|ty| format!("{} {}", format_type_name(ty), symbol.name)),
                ),
                SymbolKind::Variable { .. } => (
                    CompletionItemKind::VARIABLE,
                    symbol
                        .ty
                        .as_ref()
                        .map(|ty| format!("{} {}", format_type_name(ty), symbol.name)),
                ),
                SymbolKind::Struct => (CompletionItemKind::STRUCT, None),
                SymbolKind::StructField => (
                    CompletionItemKind::FIELD,
                    symbol
                        .ty
                        .as_ref()
                        .map(|ty| format!("{} {}", format_type_name(ty), symbol.name)),
                ),
                SymbolKind::TypeName => (CompletionItemKind::TYPE_PARAMETER, None),
                SymbolKind::Module | SymbolKind::Import => (CompletionItemKind::MODULE, None),
            };
            items.push(CompletionItem {
                label: symbol.name.clone(),
                kind: Some(kind),
                detail,
                ..Default::default()
            });
        }

        Some(items)
    }

    fn build_semantic_tokens(&self, uri: &str) -> Option<Vec<SemanticToken>> {
        let analysis = self.analysis_map.get(uri)?;
        let rope = self.document_map.get(uri)?;
        encode_semantic_tokens(&analysis, &rope, None)
    }

    fn build_semantic_tokens_range(&self, uri: &str, range: Range) -> Option<Vec<SemanticToken>> {
        let analysis = self.analysis_map.get(uri)?;
        let rope = self.document_map.get(uri)?;
        encode_semantic_tokens(&analysis, &rope, Some(range))
    }
}

struct TextDocumentChange<'a> {
    uri: String,
    text: &'a str,
}

fn encode_semantic_tokens(
    analysis: &AnalysisResult,
    rope: &Rope,
    range: Option<Range>,
) -> Option<Vec<SemanticToken>> {
    let (range_start, range_end) = if let Some(range) = range {
        (
            position_to_offset(range.start, rope)?,
            position_to_offset(range.end, rope)?,
        )
    } else {
        (0, rope.byte_len())
    };

    let mut raw_tokens = Vec::new();

    for symbol in &analysis.index.symbols {
        push_token(
            &mut raw_tokens,
            symbol.span.start_byte,
            symbol.span.end_byte.saturating_sub(symbol.span.start_byte),
            token_type_for_symbol(&symbol.kind),
            token_modifier_for_symbol(&symbol.kind),
            range_start,
            range_end,
        );
    }

    for reference in &analysis.index.references {
        if let Some(symbol) = analysis.index.symbol(reference.symbol_id) {
            push_token(
                &mut raw_tokens,
                reference.span.start_byte,
                reference
                    .span
                    .end_byte
                    .saturating_sub(reference.span.start_byte),
                token_type_for_symbol(&symbol.kind),
                0,
                range_start,
                range_end,
            );
        }
    }

    raw_tokens.sort_by_key(|token| token.0);
    Some(delta_encode_tokens(raw_tokens, rope))
}

fn push_token(
    tokens: &mut Vec<(usize, u32, u32, u32)>,
    start: usize,
    length: usize,
    token_type: u32,
    token_modifier: u32,
    range_start: usize,
    range_end: usize,
) {
    if length == 0 || start < range_start || start >= range_end {
        return;
    }
    tokens.push((start, length as u32, token_type, token_modifier));
}

fn token_type_for_symbol(kind: &SymbolKind) -> u32 {
    match kind {
        SymbolKind::Function => 0,
        SymbolKind::Variable { .. } => 1,
        SymbolKind::Parameter => 2,
        SymbolKind::Struct => 3,
        SymbolKind::StructField => 4,
        SymbolKind::TypeName => 5,
        SymbolKind::Module | SymbolKind::Import => 6,
    }
}

fn token_modifier_for_symbol(kind: &SymbolKind) -> u32 {
    match kind {
        SymbolKind::Variable { mutable: false } => 1,
        _ => 0,
    }
}

fn delta_encode_tokens(raw_tokens: Vec<(usize, u32, u32, u32)>, rope: &Rope) -> Vec<SemanticToken> {
    let mut pre_line = 0u32;
    let mut pre_start = 0u32;
    raw_tokens
        .into_iter()
        .map(|(start, length, token_type, token_modifiers_bitset)| {
            let line = rope.line_of_byte(start) as u32;
            let line_start_byte = rope.byte_of_line(line as usize);
            let char_offset = (start - line_start_byte) as u32;
            let delta_line = line - pre_line;
            let delta_start = if delta_line == 0 {
                char_offset - pre_start
            } else {
                char_offset
            };
            pre_line = line;
            pre_start = char_offset;
            SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset,
            }
        })
        .collect()
}

fn diagnostic_to_lsp(diag: &x::Diagnostic, rope: &Rope) -> Option<Diagnostic> {
    let start = position_from_span_start(&diag.span, rope)?;
    let end = position_from_span_end(&diag.span, rope)?;
    Some(Diagnostic {
        range: Range::new(start, end),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(diag.code.as_str().to_owned())),
        code_description: None,
        source: Some("xlang".to_owned()),
        message: diag.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    })
}

fn location_from_span(uri: &Url, span: x::Span, rope: &Rope) -> Option<Location> {
    let start = position_from_span_start(&span, rope)?;
    let end = position_from_span_end(&span, rope)?;
    Some(Location::new(uri.clone(), Range::new(start, end)))
}

fn position_from_span_start(span: &x::Span, rope: &Rope) -> Option<Position> {
    if span.end_byte > 0 && span.start_byte <= rope.byte_len() {
        return offset_to_position(span.start_byte, rope);
    }
    Some(Position::new(
        span.start_line.saturating_sub(1) as u32,
        span.start_column.saturating_sub(1) as u32,
    ))
}

fn position_from_span_end(span: &x::Span, rope: &Rope) -> Option<Position> {
    if span.end_byte > 0 && span.end_byte <= rope.byte_len() {
        return offset_to_position(span.end_byte, rope);
    }
    Some(Position::new(
        span.end_line.saturating_sub(1) as u32,
        span.end_column.saturating_sub(1) as u32,
    ))
}

fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
    if offset > rope.byte_len() {
        return None;
    }
    let line = rope.line_of_byte(offset);
    let line_start_byte = rope.byte_of_line(line);
    Some(Position::new(
        line as u32,
        (offset - line_start_byte) as u32,
    ))
}

fn position_to_offset(position: Position, rope: &Rope) -> Option<usize> {
    if position.line as usize >= rope.line_len() {
        return None;
    }
    Some(rope.byte_of_line(position.line as usize) + position.character as usize)
}
