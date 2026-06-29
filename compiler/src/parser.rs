use crate::ast::{
    BinaryOp, Expr, Field, Function, Import, Param, Program, Stmt, StructDecl, TypeName,
    UnaryOp,
};
use crate::diagnostic::{Diagnostic, XResult};
use crate::token::{Keyword, Token, TokenKind};

pub fn parse(tokens: Vec<Token>) -> XResult<Program> {
    Parser::new(tokens).parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn parse_program(&mut self) -> XResult<Program> {
        let module = if self.match_keyword(Keyword::Module) {
            Some(self.expect_identifier("expected module name")?)
        } else {
            None
        };

        let mut imports = Vec::new();
        while self.match_keyword(Keyword::Import) {
            let (name, span) =
                self.expect_identifier_with_span("expected import name")?;
            imports.push(Import { name, span });
        }

        let mut structs = Vec::new();
        let mut functions = Vec::new();
        while !self.is_at_end() {
            if self.check_keyword(Keyword::Struct) {
                structs.push(self.parse_struct()?);
            } else {
                functions.push(self.parse_function()?);
            }
        }

        Ok(Program {
            module,
            imports,
            structs,
            functions,
        })
    }

    fn parse_struct(&mut self) -> XResult<StructDecl> {
        let pub_export = self.parse_visibility();
        self.expect_keyword(Keyword::Struct, "expected `struct` declaration")?;
        let (name, name_span) = self.expect_identifier_with_span("expected struct name")?;
        self.expect(TokenKind::LeftBrace, "expected `{` after struct name")?;
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let (ty, ty_span) = self.parse_type_with_span()?;
            let (name, name_span) = self.expect_identifier_with_span("expected field name")?;
            self.expect(TokenKind::Semicolon, "expected `;` after field")?;
            fields.push(Field {
                name,
                name_span,
                ty,
                ty_span,
            });
        }
        self.expect(TokenKind::RightBrace, "expected `}` after struct body")?;
        Ok(StructDecl {
            pub_export,
            name,
            name_span,
            fields,
        })
    }

    fn parse_function(&mut self) -> XResult<Function> {
        let pub_export = self.parse_visibility();
        let (return_type, return_type_span) = self.parse_type_with_span()?;
        let (name, name_span) = self.expect_identifier_with_span("expected function name")?;
        self.expect(TokenKind::LeftParen, "expected `(` after function name")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let (ty, ty_span) = self.parse_type_with_span()?;
                let (name, name_span) =
                    self.expect_identifier_with_span("expected parameter name")?;
                params.push(Param {
                    name,
                    name_span,
                    ty,
                    ty_span,
                });
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "expected `)` after parameters")?;
        let body = self.parse_block()?;
        Ok(Function {
            pub_export,
            name,
            name_span,
            params,
            return_type,
            return_type_span: Some(return_type_span),
            body,
        })
    }

    fn parse_visibility(&mut self) -> bool {
        self.match_keyword(Keyword::Pub)
    }

    fn parse_block(&mut self) -> XResult<Vec<Stmt>> {
        self.expect(TokenKind::LeftBrace, "expected `{`")?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RightBrace, "expected `}`")?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> XResult<Stmt> {
        if self.match_keyword(Keyword::Const) {
            return self.parse_binding(false, "expected binding name after type");
        }
        if self.check_keyword(Keyword::Return) {
            let keyword_span = self.advance().span;
            let expr = if self.check(&TokenKind::Semicolon) {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect(TokenKind::Semicolon, "expected `;` after return statement")?;
            return Ok(Stmt::Return {
                value: expr,
                keyword_span,
            });
        }
        if self.check_keyword(Keyword::Break) {
            let keyword_span = self.advance().span;
            self.expect(TokenKind::Semicolon, "expected `;` after break statement")?;
            return Ok(Stmt::Break { keyword_span });
        }
        if self.check_keyword(Keyword::Continue) {
            let keyword_span = self.advance().span;
            self.expect(
                TokenKind::Semicolon,
                "expected `;` after continue statement",
            )?;
            return Ok(Stmt::Continue { keyword_span });
        }
        if self.check_keyword(Keyword::While) {
            let keyword_span = self.advance().span;
            let condition = self.parse_expr()?;
            let body = self.parse_block()?;
            return Ok(Stmt::While {
                condition,
                keyword_span,
                body,
            });
        }
        if self.check_keyword(Keyword::If) {
            let keyword_span = self.advance().span;
            let condition = self.parse_expr()?;
            let then_body = self.parse_block()?;
            let else_body = if self.match_keyword(Keyword::Else) {
                self.parse_block()?
            } else {
                Vec::new()
            };
            return Ok(Stmt::If {
                condition,
                keyword_span,
                then_body,
                else_body,
            });
        }
        if self.starts_typed_binding() {
            return self.parse_binding(true, "expected binding name after type");
        }
        if let Some((name, name_span)) = self.peek_identifier()
            && self.peek_next_kind_is(&TokenKind::Dot)
        {
            self.advance();
            self.advance();
            let (field, field_span) =
                self.expect_identifier_with_span("expected field name after `.`")?;
            self.expect(TokenKind::Equal, "expected `=` in field assignment")?;
            let value = self.parse_expr()?;
            self.expect(TokenKind::Semicolon, "expected `;` after assignment")?;
            return Ok(Stmt::AssignField {
                name,
                name_span,
                field,
                field_span,
                value,
            });
        }
        if let Some((name, name_span)) = self.peek_identifier()
            && self.peek_next_kind_is(&TokenKind::LeftBracket)
        {
            self.advance();
            self.advance();
            let index = self.parse_expr()?;
            self.expect(TokenKind::RightBracket, "expected `]` after index")?;
            self.expect(TokenKind::Equal, "expected `=` in assignment")?;
            let value = self.parse_expr()?;
            self.expect(TokenKind::Semicolon, "expected `;` after assignment")?;
            return Ok(Stmt::AssignIndex {
                name,
                name_span,
                index,
                value,
            });
        }
        if let Some((name, name_span)) = self.peek_identifier()
            && self.peek_next_kind_is(&TokenKind::Equal)
        {
            self.advance();
            self.advance();
            let value = self.parse_expr()?;
            self.expect(TokenKind::Semicolon, "expected `;` after assignment")?;
            return Ok(Stmt::Assign {
                name,
                name_span,
                value,
            });
        }
        let expr = self.parse_expr()?;
        self.expect(
            TokenKind::Semicolon,
            "expected `;` after expression statement",
        )?;
        Ok(Stmt::Expr(expr))
    }

    fn parse_binding(&mut self, mutable: bool, name_message: &str) -> XResult<Stmt> {
        let (annotation, annotation_span) = self.parse_type_with_span()?;
        let (name, name_span) = self.expect_identifier_with_span(name_message)?;
        self.expect(TokenKind::Equal, "expected `=` in binding")?;
        let value = if matches!(annotation, TypeName::Named(_)) {
            self.parse_struct_literal()?
        } else {
            self.parse_expr()?
        };
        self.expect(TokenKind::Semicolon, "expected `;` after binding")?;
        Ok(Stmt::Let {
            mutable,
            name,
            name_span,
            annotation: Some(annotation),
            annotation_span: Some(annotation_span),
            value,
        })
    }

    fn parse_struct_literal(&mut self) -> XResult<Expr> {
        self.expect(TokenKind::LeftBrace, "expected `{` to start struct literal")?;
        let start = self.tokens[self.index - 1].span;
        let mut elements = Vec::new();
        if !self.check(&TokenKind::RightBrace) {
            loop {
                elements.push(self.parse_expr()?);
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        let end = self.current().span;
        self.expect(TokenKind::RightBrace, "expected `}` after struct literal")?;
        Ok(Expr::StructLiteral {
            elements,
            span: start.join(end),
        })
    }

    fn parse_type_with_span(&mut self) -> XResult<(TypeName, crate::diagnostic::Span)> {
        let (base, span) = self.parse_base_type_with_span()?;
        if !self.match_token(&TokenKind::LeftBracket) {
            return Ok((base, span));
        }
        let len_token = self.advance().clone();
        let len = match len_token.kind {
            TokenKind::Integer(value) => value,
            _ => {
                return Err(Diagnostic::parse(
                    "expected array length literal",
                    len_token.line(),
                    len_token.column(),
                ));
            }
        };
        self.expect(TokenKind::RightBracket, "expected `]` after array length")?;
        Ok((
            TypeName::Array {
                elem: Box::new(base),
                len: len as usize,
            },
            span,
        ))
    }

    fn parse_base_type_with_span(&mut self) -> XResult<(TypeName, crate::diagnostic::Span)> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(name) => match name.as_str() {
                "i32" => Ok((TypeName::I32, token.span)),
                "bool" => Ok((TypeName::Bool, token.span)),
                "str" => Ok((TypeName::Str, token.span)),
                "void" => Ok((TypeName::Void, token.span)),
                _ if self.match_token(&TokenKind::Dot) => {
                    let (struct_name, name_span) =
                        self.expect_identifier_with_span("expected struct name after `.`")?;
                    Ok((
                        TypeName::Qualified {
                            module: name,
                            name: struct_name,
                        },
                        token.span.join(name_span),
                    ))
                }
                _ => Ok((TypeName::Named(name), token.span)),
            },
            _ => Err(Diagnostic::parse(
                "expected type name",
                token.line(),
                token.column(),
            )),
        }
    }

    fn parse_expr(&mut self) -> XResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> XResult<Expr> {
        let mut expr = self.parse_and()?;
        while self.match_token(&TokenKind::PipePipe) {
            let left_span = expr.span();
            let right = self.parse_and()?;
            let span = left_span.join(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> XResult<Expr> {
        let mut expr = self.parse_equality()?;
        while self.match_token(&TokenKind::AmpAmp) {
            let left_span = expr.span();
            let right = self.parse_equality()?;
            let span = left_span.join(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> XResult<Expr> {
        let mut expr = self.parse_comparison()?;
        loop {
            let op = if self.match_token(&TokenKind::EqualEqual) {
                BinaryOp::Equal
            } else if self.match_token(&TokenKind::BangEqual) {
                BinaryOp::NotEqual
            } else {
                break;
            };
            let left_span = expr.span();
            let right = self.parse_comparison()?;
            let span = left_span.join(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> XResult<Expr> {
        let mut expr = self.parse_term()?;
        loop {
            let op = if self.match_token(&TokenKind::Less) {
                BinaryOp::Less
            } else if self.match_token(&TokenKind::LessEqual) {
                BinaryOp::LessEqual
            } else if self.match_token(&TokenKind::Greater) {
                BinaryOp::Greater
            } else if self.match_token(&TokenKind::GreaterEqual) {
                BinaryOp::GreaterEqual
            } else {
                break;
            };
            let left_span = expr.span();
            let right = self.parse_term()?;
            let span = left_span.join(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> XResult<Expr> {
        let mut expr = self.parse_factor()?;
        loop {
            let op = if self.match_token(&TokenKind::Plus) {
                BinaryOp::Add
            } else if self.match_token(&TokenKind::Minus) {
                BinaryOp::Subtract
            } else {
                break;
            };
            let left_span = expr.span();
            let right = self.parse_factor()?;
            let span = left_span.join(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> XResult<Expr> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.match_token(&TokenKind::Star) {
                BinaryOp::Multiply
            } else if self.match_token(&TokenKind::Slash) {
                BinaryOp::Divide
            } else if self.match_token(&TokenKind::Percent) {
                BinaryOp::Remainder
            } else {
                break;
            };
            let left_span = expr.span();
            let right = self.parse_unary()?;
            let span = left_span.join(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> XResult<Expr> {
        if self.check(&TokenKind::Minus) {
            let start = self.advance().span;
            let expr = self.parse_unary()?;
            let span = start.join(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(expr),
                span,
            });
        }
        if self.check(&TokenKind::Bang) {
            let start = self.advance().span;
            let expr = self.parse_unary()?;
            let span = start.join(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
                span,
            });
        }
        let primary = self.parse_primary()?;
        self.parse_postfix(primary)
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> XResult<Expr> {
        loop {
            if self.match_token(&TokenKind::LeftBracket) {
                let index = self.parse_expr()?;
                let end = self.current().span;
                self.expect(TokenKind::RightBracket, "expected `]` after index")?;
                let span = expr.span().join(index.span()).join(end);
                expr = Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else if self.match_token(&TokenKind::Dot) {
                let (field, field_span) =
                    self.expect_identifier_with_span("expected field name after `.`")?;
                let span = expr.span().join(field_span);
                expr = Expr::FieldAccess {
                    base: Box::new(expr),
                    field,
                    field_span,
                    span,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> XResult<Expr> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Integer {
                value,
                span: token.span,
            }),
            TokenKind::String(value) => Ok(Expr::String {
                value,
                span: token.span,
            }),
            TokenKind::Keyword(Keyword::True) => Ok(Expr::Bool {
                value: true,
                span: token.span,
            }),
            TokenKind::Keyword(Keyword::False) => Ok(Expr::Bool {
                value: false,
                span: token.span,
            }),
            TokenKind::Identifier(name) => {
                if self.match_token(&TokenKind::LeftParen) {
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.match_token(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    let end = self.current().span;
                    self.expect(TokenKind::RightParen, "expected `)` after arguments")?;
                    Ok(Expr::Call {
                        callee: name,
                        args,
                        span: token.span.join(end),
                    })
                } else if self.peek_qualified_call() {
                    self.advance();
                    let (callee, callee_span) =
                        self.expect_identifier_with_span("expected function name after `.`")?;
                    self.expect(
                        TokenKind::LeftParen,
                        "expected `(` after qualified function name",
                    )?;
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.match_token(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    let end = self.current().span;
                    self.expect(TokenKind::RightParen, "expected `)` after arguments")?;
                    Ok(Expr::QualifiedCall {
                        module: name,
                        callee,
                        args,
                        span: token.span.join(callee_span).join(end),
                    })
                } else {
                    Ok(Expr::Variable {
                        name,
                        span: token.span,
                    })
                }
            }
            TokenKind::LeftBrace => {
                let start = token.span;
                let mut elements = Vec::new();
                if !self.check(&TokenKind::RightBrace) {
                    loop {
                        elements.push(self.parse_expr()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end = self.current().span;
                self.expect(TokenKind::RightBrace, "expected `}` after array literal")?;
                Ok(Expr::ArrayLiteral {
                    elements,
                    span: start.join(end),
                })
            }
            TokenKind::LeftParen => {
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RightParen, "expected `)` after expression")?;
                Ok(expr)
            }
            _ => Err(Diagnostic::parse(
                "expected expression",
                token.line(),
                token.column(),
            )),
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword, message: &str) -> XResult<()> {
        if self.match_keyword(keyword) {
            Ok(())
        } else {
            let token = self.current();
            Err(Diagnostic::parse(message, token.line(), token.column()))
        }
    }

    fn expect_identifier(&mut self, message: &str) -> XResult<String> {
        self.expect_identifier_with_span(message)
            .map(|(name, _)| name)
    }

    fn expect_identifier_with_span(
        &mut self,
        message: &str,
    ) -> XResult<(String, crate::diagnostic::Span)> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(name) => Ok((name, token.span)),
            _ => Err(Diagnostic::parse(message, token.line(), token.column())),
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> XResult<()> {
        if self.match_token(&kind) {
            Ok(())
        } else {
            let token = self.current();
            Err(Diagnostic::parse(message, token.line(), token.column()))
        }
    }

    fn match_keyword(&mut self, keyword: Keyword) -> bool {
        if self.check_keyword(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(k) if k == keyword)
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.index += 1;
        }
        &self.tokens[self.index - 1]
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn peek_qualified_call(&self) -> bool {
        if !self.check(&TokenKind::Dot) {
            return false;
        }
        let callee = self.tokens.get(self.index + 1).map(|token| &token.kind);
        let after = self.tokens.get(self.index + 2).map(|token| &token.kind);
        matches!(callee, Some(TokenKind::Identifier(_)))
            && matches!(after, Some(TokenKind::LeftParen))
    }

    fn peek_identifier(&self) -> Option<(String, crate::diagnostic::Span)> {
        match &self.current().kind {
            TokenKind::Identifier(name) => Some((name.clone(), self.current().span)),
            _ => None,
        }
    }

    fn peek_next_kind_is(&self, kind: &TokenKind) -> bool {
        self.tokens
            .get(self.index + 1)
            .map(|token| std::mem::discriminant(&token.kind) == std::mem::discriminant(kind))
            .unwrap_or(false)
    }

    fn starts_typed_binding(&self) -> bool {
        let TokenKind::Identifier(name) = &self.current().kind else {
            return false;
        };
        if self.peek_next_kind_is(&TokenKind::Dot) {
            return self
                .tokens
                .get(self.index + 2)
                .map(|token| matches!(token.kind, TokenKind::Identifier(_)))
                .unwrap_or(false)
                && self
                    .tokens
                    .get(self.index + 3)
                    .map(|token| matches!(token.kind, TokenKind::Identifier(_)))
                    .unwrap_or(false);
        }
        match self.tokens.get(self.index + 1).map(|token| &token.kind) {
            Some(TokenKind::Identifier(_)) => true,
            Some(TokenKind::LeftBracket) => Self::is_type_name_spelling(name),
            _ => false,
        }
    }

    fn is_type_name_spelling(name: &str) -> bool {
        matches!(name, "i32" | "bool" | "void" | "str")
    }
}
