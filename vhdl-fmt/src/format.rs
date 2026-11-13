// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

//! Turns a syntax tree into a layout document.
//!
//! [`Emitter::node`] dispatches on the node kind. Structural rules decide
//! where a declaration, statement or closing keyword starts a line and how
//! deep it is indented. Layout rules add the places where a line may break
//! if it does not fit. Every other node emits its tokens in source order.
//! Because the tree is lossless, a missing rule can cost layout quality but
//! can never drop or reorder a token.
//!
//! The source's own line breaks are not kept, with two exceptions: a blank
//! line between declarations or statements survives, and a comment stays on
//! the line it was written on.

use crate::config::{FormatConfig, KeywordCase};
use crate::layout::{self, Doc};
use vhdl_syntax::standard::VHDLStandard;
use vhdl_syntax::syntax::{NodeKind, SyntaxElement, SyntaxNode, SyntaxToken};
use vhdl_syntax::tokens::token::requires_separator;
use vhdl_syntax::tokens::{Keyword, TokenKind, TriviaPiece};

/// Source bytes are Latin-1, except comments, which may hold any encoding.
/// Mapping each byte to the `char` with the same value keeps every byte
/// recoverable whatever the encoding; [`encode`] is the exact inverse. The
/// cost is that a multi-byte UTF-8 character counts as several columns.
pub(crate) fn decode(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| char::from(byte)).collect()
}

pub(crate) fn encode(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| u8::try_from(u32::from(ch)).expect("formatter text is decoded from bytes"))
        .collect()
}

/// The text of a comment as the formatter writes it, delimiters included.
/// Line comments lose trailing whitespace; block comments are verbatim apart
/// from line endings, which are a property of the file.
pub(crate) fn comment_text(piece: &TriviaPiece) -> Option<String> {
    match piece {
        TriviaPiece::LineComment(comment) => Some(decode(comment.as_bytes()).trim_end().to_owned()),
        TriviaPiece::BlockComment(comment) => Some(decode(comment.as_bytes()).replace('\r', "")),
        _ => None,
    }
}

/// `-- fmt: off` and `-- fmt: on` delimit source that is copied verbatim.
/// `-- fmt: skip` does the same for one declaration or statement.
fn directive(comment: &str) -> Option<&str> {
    let body = comment.strip_prefix("--")?.trim();
    matches!(body, "fmt: off" | "fmt: on" | "fmt: skip").then_some(body)
}

fn is_skip(piece: &TriviaPiece) -> bool {
    matches!(piece, TriviaPiece::LineComment(_))
        && comment_text(piece).is_some_and(|text| directive(&text) == Some("fmt: skip"))
}

/// A declaration or statement is skipped by `-- fmt: skip` on the line above
/// it, or at the end of its last line.
fn is_skipped(item: &SyntaxNode) -> bool {
    // A comment before the first line break trails the previous item.
    let first = item.first_token();
    let trivia = first.leading_trivia();
    let above = trivia
        .iter()
        .rposition(|piece| piece.is_comment())
        .is_some_and(|comment| {
            let own_line = first.prev_token().is_none()
                || trivia.iter().take(comment).any(|piece| piece.is_newline());
            own_line && trivia.get(comment).is_some_and(is_skip)
        });
    let after = item.last_token().next_token().is_some_and(|next| {
        next.leading_trivia()
            .iter()
            .take_while(|piece| !piece.is_newline())
            .any(is_skip)
    });
    above || after
}

/// Whitespace seen since the last emitted token or comment.
#[derive(Default)]
struct Gap {
    newlines: usize,
    /// Width of the whitespace after the last newline.
    columns: usize,
}

fn ends_operand(kind: TokenKind) -> bool {
    use TokenKind::*;
    matches!(
        kind,
        Identifier
            | AbstractLiteral
            | StringLiteral
            | BitStringLiteral
            | CharacterLiteral
            | RightPar
            | RightSquare
            | Keyword(self::Keyword::All)
    )
}

/// Default spacing between two tokens on one line, decided from token kinds
/// alone. `before_previous` distinguishes attribute names that are reserved
/// words (`x'range(1)`) from keywords (`array (1 to 2)`).
fn space_between(
    before_previous: Option<TokenKind>,
    previous: TokenKind,
    previous_is_sign: bool,
    next: TokenKind,
) -> bool {
    use TokenKind::*;
    if matches!(next, SemiColon | Comma | RightPar | RightSquare | Colon)
        || matches!(previous, LeftPar | LeftSquare | Dot | CommAt)
        || previous == Tick
        || next == Tick
        || previous_is_sign
        // The matching forms `case?` and `select?`.
        || (next == Que && matches!(previous, Keyword(self::Keyword::Case | self::Keyword::Select)))
    {
        return false;
    }
    match next {
        Dot => !(ends_operand(previous) || previous == Circ),
        LeftPar => {
            let attribute = matches!(previous, Keyword(_)) && before_previous == Some(Tick);
            let header = matches!(
                previous,
                Keyword(self::Keyword::Process | self::Keyword::Block | self::Keyword::Abs)
            );
            !(ends_operand(previous) || header || attribute)
        }
        LeftSquare => !ends_operand(previous),
        _ => true,
    }
}

/// Nodes whose child nodes each start a line, one level deeper.
fn is_block(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        ArchitectureDeclarativePart
            | BlockDeclarativePart
            | ConfigurationDeclarativePart
            | EntityDeclarativePart
            | PackageDeclarativePart
            | PackageBodyDeclarativePart
            | ProcessDeclarativePart
            | ProtectedTypeDeclarativePart
            | ProtectedTypeBodyDeclarativePart
            | SubprogramDeclarativePart
            | ArchitectureStatementPart
            | BlockStatementPart
            | EntityStatementPart
            | ProcessStatementPart
            | SubprogramStatementPart
            | SequenceOfStatements
            | GenerateStatementBody
            | RecordElementDeclarations
            | EntityHeader
            | PackageHeader
            | GenericPart
            | PortPart
    )
}

/// Children of a block that are not items themselves: their own children
/// decide where lines start.
fn is_transparent(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        ContextClause | GenerateBodyDeclarations | GenerateBodyEpilogue
    )
}

/// Nodes that start a line at the level of the construct they belong to,
/// such as `begin`, `elsif` and `end ...`.
fn starts_line(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        DeclarationStatementSeparator
            | IfStatementElsif
            | IfStatementElse
            | IfGenerateElsif
            | IfGenerateElse
            | ArchitectureEpilogue
            | BlockConfigurationEpilogue
            | BlockEpilogue
            | CaseStatementEpilogue
            | ComponentConfigurationEpilogue
            | ComponentDeclarationEpilogue
            | ConfigurationDeclarationEpilogue
            | ContextDeclarationEpilogue
            | EntityDeclarationEpilogue
            | GenerateBodyEpilogue
            | GenerateEpilogue
            | IfStatementEpilogue
            | LoopStatementEpilogue
            | PackageBodyEpilogue
            | PackageEpilogue
            | ProcessEpilogue
            | ProtectedTypeBodyEpilogue
            | ProtectedTypeDeclarationEpilogue
            | RecordTypeDefinitionEpilogue
            | SubprogramBodyEpilogue
    )
}

/// Nodes that start a line one level below the construct they belong to.
fn starts_indented_line(kind: NodeKind, parent: Option<NodeKind>) -> bool {
    use NodeKind::*;
    match kind {
        CaseStatementAlternative
        | CaseGenerateAlternative
        | PhysicalTypeDefinitionEpilogue
        | BlockConfiguration
        | ComponentConfiguration => true,
        GenericClause | PortClause => parent == Some(ComponentDeclaration),
        GenericMapAspect | PortMapAspect => parent == Some(ComponentInstantiationStatement),
        Binding => parent == Some(ComponentConfiguration),
        _ => false,
    }
}

/// Nodes laid out as one group: either all of their breaks are taken, or
/// none.
fn is_group(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        ConditionalWaveforms
            | ConditionalExpressions
            | SelectedWaveforms
            | SelectedExpressions
            | WaveformElements
            | Choices
            | IdentifierList
            | WaitStatement
    )
}

/// Nodes that move to a new line, one level deeper, when the group around
/// them does not fit.
fn breaks_before(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        ElseWaveform
            | ElseWhenWaveform
            | ElseExpression
            | ElseWhenExpression
            | AfterClause
            | SeverityClause
            | SensitivityClause
            | ConditionClause
            | TimeoutClause
    )
}

/// The keyword that ends the header of a compound statement. If the header
/// does not fit on one line, the keyword starts a line of its own.
fn header_end(kind: NodeKind) -> Option<Keyword> {
    use NodeKind::*;
    match kind {
        IfStatementPreamble | IfStatementElsif => Some(Keyword::Then),
        LoopStatementPreamble => Some(Keyword::Loop),
        ForGeneratePreamble | IfGenerateIf | IfGenerateElsif => Some(Keyword::Generate),
        _ => None,
    }
}

/// Statements and declarations in which the text after `<=` or `:=` moves to
/// a new line, one level deeper, if it does not fit after the operator.
fn is_assignment(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        SimpleVariableAssignment
            | ConditionalVariableAssignment
            | SimpleWaveformAssignment
            | ConditionalWaveformAssignment
            | SimpleForceAssignment
            | ConditionalForceAssignment
            | ConcurrentSimpleSignalAssignment
            | ConcurrentConditionalSignalAssignment
            | InitialValue
    )
}

/// Declarations whose colon follows the `space_before_colon` setting. The
/// colon of a label never has a space before it.
fn is_declaration_colon(parent: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        parent,
        ConstantDeclaration
            | SignalDeclaration
            | VariableDeclaration
            | FileDeclaration
            | ElementDeclaration
            | InterfaceObjectDeclaration
            | InterfaceFileDeclaration
            | AliasSubtype
            | AttributeDeclaration
            | EntitySpecification
    )
}

/// A cell that is this much wider than the median of its column is not
/// aligned with: the padding of the other rows would exceed it.
const MAX_ALIGNMENT_GAP: usize = 40;

/// A row of an aligned run: where its document is and how wide its cells are.
struct AlignedRow {
    index: usize,
    widths: Vec<usize>,
    /// Width of the row on one line.
    flat: usize,
    /// Width of the first line if the row wraps after its last column.
    head: Option<usize>,
}

impl AlignedRow {
    /// With `padding` added, the row fits on one line, or can wrap without
    /// moving a column off its first line.
    fn fits(&self, padding: usize, available: usize) -> bool {
        self.flat + padding <= available
            || self.head.is_some_and(|head| head + padding <= available)
    }
}

/// A line break requested by a rule before the next token.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Break {
    /// A new line (1) or a blank line (2). Where the line is, in its block,
    /// decides whether a blank line of the source is kept.
    Hard(usize, Edge),
    /// A break that is taken only if the enclosing group does not fit on one
    /// line. Otherwise it is a space, or nothing.
    Soft { space: bool },
    /// Like a soft break with a space, but if the group that follows fits on
    /// a line of its own, it moves there whole before it is broken up.
    Preferred,
}

/// The position of a line in the block that holds it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    /// Between two declarations or statements: a blank line above is kept.
    Inside,
    /// First in the block: no blank line above.
    First,
    /// Closes the block or one of its parts, like `begin`, `else` or `end`:
    /// no blank line directly above, but one above a comment is kept.
    Closing,
}

/// Names of the predefined attributes. They are not reserved words, but
/// follow the keyword case like `range` and `subtype`, which are.
const PREDEFINED_ATTRIBUTES: &[&str] = &[
    "active",
    "ascending",
    "base",
    "converse",
    "delayed",
    "designated_subtype",
    "driving",
    "driving_value",
    "element",
    "event",
    "high",
    "image",
    "index",
    "instance_name",
    "last_active",
    "last_event",
    "last_value",
    "left",
    "leftof",
    "length",
    "low",
    "path_name",
    "pos",
    "pred",
    "quiet",
    "reflect",
    "reverse_range",
    "right",
    "rightof",
    "simple_name",
    "stable",
    "succ",
    "transaction",
    "val",
    "value",
];

/// Whether an identifier after a tick is cased like a keyword.
pub(crate) fn is_predefined_attribute(name: &[u8]) -> bool {
    let name = decode(name).to_ascii_lowercase();
    PREDEFINED_ATTRIBUTES.contains(&name.as_str())
}

/// Stands for the space before a comment at the end of a line of code until
/// the lines are rendered and such comments can be lined up. Source text is
/// decoded from bytes, so it cannot contain this character.
const COMMENT_GAP: char = '\u{E000}';

/// How the breaks of a group are decided.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Each break on its own: taken if the text up to the next one does not
    /// fit.
    Fill,
    /// All or none. Lines are indented at the levels they were emitted with.
    Together,
    /// All or none. If the group starts in the middle of a line, its lines
    /// are indented relative to that line.
    Anchored,
}

struct Forced {
    kind: Break,
    /// The break has been emitted, before the token or a comment above it.
    done: bool,
}

/// How tightly a binary operator binds. Operators of one class share a group
/// when chained, so a chain breaks before every operator or before none.
fn operator_class(kind: TokenKind) -> u8 {
    use self::Keyword::*;
    use TokenKind::*;
    match kind {
        Keyword(And | Or | Nand | Nor | Xor | Xnor) => 1,
        EQ | NE | LT | LTE | GT | GTE | QueEQ | QueNE | QueLT | QueLTE | QueGT | QueGTE => 2,
        Keyword(Sll | Srl | Sla | Sra | Rol | Ror) => 3,
        Plus | Minus => 4,
        Concat => 5,
        Times | Div | Keyword(Mod | Rem) => 6,
        Pow => 7,
        _ => 0,
    }
}

pub(crate) struct Emitter<'a> {
    config: &'a FormatConfig,
    standard: VHDLStandard,
    docs: Vec<Doc>,
    has_content: bool,
    previous: Option<SyntaxToken>,
    before_previous: Option<TokenKind>,
    /// The previous token is a unary `+` or `-`.
    previous_is_sign: bool,
    /// The last emitted item is a comment rather than `previous`.
    after_comment: bool,
    /// The last emitted item is a comment that ended its line in the source,
    /// so the next item starts a new one.
    after_closing_comment: bool,
    /// Verbatim source collected after `-- fmt: off`.
    raw: Option<String>,
    /// Indentation level of a line that starts at the next token.
    level: usize,
    /// Line break requested before the next token.
    pending: Option<Break>,
    /// Groups that open at the next token, after its comments and the break
    /// before it, so that neither makes the group look broken.
    deferred_groups: usize,
    /// Start, in `docs`, of each open group.
    group_starts: Vec<usize>,
    /// Level of the line that the last token started, if it started one.
    line_start_level: Option<usize>,
    /// Inside an external name, where the colon has a space on both sides.
    spaced_colon: usize,
    /// Tokens of the rows being emitted that start an aligned column, by
    /// source offset, each with the least number of spaces before it.
    align_targets: Vec<(usize, usize)>,
    /// The token being emitted starts an aligned column, after at least this
    /// many spaces.
    align: Option<usize>,
    /// The pending break belongs inside the group that opens at the next
    /// token: this many of the deferred groups open before the break.
    break_inside_group: usize,
}

impl<'a> Emitter<'a> {
    pub(crate) fn new(config: &'a FormatConfig, standard: VHDLStandard) -> Self {
        Self {
            config,
            standard,
            docs: Vec::new(),
            has_content: false,
            previous: None,
            before_previous: None,
            previous_is_sign: false,
            after_comment: false,
            after_closing_comment: false,
            raw: None,
            level: 0,
            pending: None,
            deferred_groups: 0,
            group_starts: Vec::new(),
            line_start_level: None,
            spaced_colon: 0,
            align_targets: Vec::new(),
            align: None,
            break_inside_group: 0,
        }
    }

    /// Decide where a node starts, then lay out what is inside it.
    pub(crate) fn node(&mut self, node: &SyntaxNode) {
        use NodeKind::*;
        let kind = node.kind();
        let parent = node.parent().map(|parent| parent.kind());
        match kind {
            // Design units are separated by a blank line.
            DesignFile => self.block(node, 0, 2),
            DesignUnit => self.block(node, 0, 1),
            ContextClause => self.context_clause(node, parent),
            // `units` sits one level below the type; the units below that.
            UnitDeclarations => {
                self.level += 1;
                self.break_line(1);
                self.block(node, 1, 1);
                self.level -= 1;
            }
            _ if is_block(kind) => self.block(node, 1, 1),
            _ if starts_line(kind) => {
                self.break_line_at(1, Edge::Closing);
                self.inner(node);
            }
            _ if starts_indented_line(kind, parent) => {
                // The first alternative or clause follows its header directly.
                let after_header = node
                    .prev_sibling()
                    .is_some_and(|previous| format!("{:?}", previous.kind()).ends_with("Preamble"));
                let edge = if kind == PhysicalTypeDefinitionEpilogue {
                    Edge::Closing
                } else if after_header {
                    Edge::First
                } else {
                    Edge::Inside
                };
                self.level += 1;
                self.break_line_at(1, edge);
                self.inner(node);
                self.level -= 1;
            }
            // The first clause of a wait statement stays next to `wait`.
            SensitivityClause | ConditionClause | TimeoutClause
                if node.prev_sibling_or_token().is_some_and(|previous| {
                    matches!(previous, SyntaxElement::Token(token)
                            if token.kind() == TokenKind::Keyword(Keyword::Wait))
                }) =>
            {
                self.inner(node)
            }
            // An assertion that does not fit on one line gives the report
            // and the severity a line each.
            ReportClause if parent == Some(Assertion) => {
                self.level += 1;
                self.soft_break(true);
                self.inner(node);
                self.level -= 1;
            }
            _ if breaks_before(kind) => {
                self.level += 1;
                self.soft_break(true);
                self.inner(node);
                self.level -= 1;
            }
            _ => self.inner(node),
        }
    }

    fn inner(&mut self, node: &SyntaxNode) {
        use NodeKind::*;
        let kind = node.kind();
        match kind {
            GenericClause
            | PortClause
            | ParenthesizedInterfaceList
            | GenericMapAspect
            | PortMapAspect
            | ParenthesizedExpressionOrAggregate
            | Aggregate => self.delimited(node, true),
            // Calls, indexed names, enumerations and sensitivity lists are
            // limited by the line width alone.
            ParenthesizedName | EnumerationTypeDefinition | ParenthesizedProcessSensitivityList => {
                self.delimited(node, false)
            }
            ExternalConstantName | ExternalSignalName | ExternalVariableName => {
                self.spaced_colon += 1;
                self.grouped(Shape::Together, |emitter| emitter.external_name(node));
                self.spaced_colon -= 1;
            }
            BinaryExpression => {
                let class = Self::binary_class(node);
                self.grouped(Shape::Anchored, |emitter| emitter.binary_chain(node, class));
            }
            _ if is_group(kind) => {
                let shape = match kind {
                    ConditionalWaveforms | ConditionalExpressions => Shape::Anchored,
                    _ => Shape::Together,
                };
                self.grouped(shape, |emitter| emitter.separated(node));
            }
            _ if is_assignment(kind) => self.after_token(node, |kind| {
                matches!(kind, TokenKind::ColonEq | TokenKind::LTE)
            }),
            ReturnStatement => {
                self.after_token(node, |kind| kind == TokenKind::Keyword(Keyword::Return))
            }
            // The message stays next to `report`. If the statement does not
            // fit on one line, the severity gets a line of its own.
            ReportStatement | Assertion => {
                self.grouped(Shape::Together, |emitter| emitter.children(node));
            }
            WhenWaveform | ElseWhenWaveform | WhenExpression | ElseWhenExpression => {
                self.conditional_item(node)
            }
            SubtypeDeclaration => {
                self.after_token(node, |kind| kind == TokenKind::Keyword(Keyword::Is))
            }
            // A specification or alias that does not fit breaks at every one
            // of these places.
            AttributeSpecification => {
                self.grouped(Shape::Together, |emitter| {
                    emitter.after_each(node, |kind| {
                        matches!(kind, TokenKind::Keyword(Keyword::Of | Keyword::Is))
                    })
                });
            }
            AliasDeclaration => {
                self.grouped(Shape::Together, |emitter| {
                    emitter.after_each(node, |kind| {
                        matches!(kind, TokenKind::Colon | TokenKind::Keyword(Keyword::Is))
                    })
                });
            }
            ConstantDeclaration
            | SignalDeclaration
            | VariableDeclaration
            | FileDeclaration
            | ElementDeclaration
            | InterfaceObjectDeclaration
            | InterfaceFileDeclaration => self.declaration(node),
            SelectedVariableAssignment
            | SelectedWaveformAssignment
            | SelectedForceAssignment
            | ConcurrentSelectedSignalAssignment => self.selected_assignment(node),
            FunctionSpecification => self.before_keyword(node, Keyword::Return),
            UnboundedArrayDefinition => self.delimited(node, false),
            ConstrainedArrayDefinition => self.before_keyword(node, Keyword::Of),
            SelectedWaveformItem | SelectedExpressionItem => {
                self.before_keyword(node, Keyword::When)
            }
            _ => match header_end(kind) {
                Some(keyword) => self.header(node, keyword),
                None => self.children(node),
            },
        }
    }

    /// Emit the children in source order without adding any break.
    fn children(&mut self, node: &SyntaxNode) {
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(node) => self.node(&node),
                SyntaxElement::Token(token) => self.token(&token),
            }
        }
    }

    /// Start each child node on its own line, `indent` levels deeper.
    fn block(&mut self, node: &SyntaxNode, indent: usize, newlines: usize) {
        let mut run = Vec::new();
        let level = self.level + indent;
        let mut previous = None;
        let mut first = true;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(child) if is_transparent(child.kind()) => {
                    previous = Some(child.kind());
                    self.node(&child)
                }
                SyntaxElement::Node(child) => {
                    let targets = self.block_row_targets(&child);
                    // Attributes are aligned with each other, not with the
                    // object declarations next to them.
                    let is_attribute = |kind| {
                        matches!(
                            kind,
                            NodeKind::AttributeDeclaration | NodeKind::AttributeSpecification
                        )
                    };
                    let other_family = previous.is_some_and(|previous| {
                        is_attribute(previous) != is_attribute(child.kind())
                    });
                    if targets.is_empty() || other_family || Self::starts_new_run(&child) {
                        self.align_rows(&mut run, level, false);
                    }
                    let is_row = !targets.is_empty();
                    // Processes and subprogram bodies stand apart from their
                    // neighbours, and a design unit from its context clause.
                    let apart = |kind| {
                        matches!(kind, NodeKind::ProcessStatement | NodeKind::SubprogramBody)
                    };
                    let separated = previous.is_some_and(|previous| {
                        apart(previous)
                            || apart(child.kind())
                            || node.kind() == NodeKind::DesignUnit
                    });
                    let newlines = if separated { 2 } else { newlines };
                    previous = Some(child.kind());
                    if std::mem::take(&mut first) && indent > 0 {
                        self.break_line_at(1, Edge::First);
                    }
                    let row = self.with_targets(targets, |emitter| {
                        emitter.item_node(&child, indent, newlines)
                    });
                    run.extend(row.filter(|_| is_row));
                }
                SyntaxElement::Token(token) => self.token(&token),
            }
        }
        self.align_rows(&mut run, level, false);
    }

    /// Emit a row whose `targets` start aligned columns.
    fn with_targets<R>(
        &mut self,
        targets: Vec<(usize, usize)>,
        emit: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let outer = self.align_targets.len();
        self.align_targets.extend(targets);
        let result = emit(self);
        self.align_targets.truncate(outer);
        result
    }

    /// A blank line or a comment above an item ends the run of rows that are
    /// aligned with each other.
    fn starts_new_run(item: &SyntaxNode) -> bool {
        let mut newlines = 0;
        for piece in item.first_token().leading_trivia().iter() {
            if piece.is_comment() {
                if newlines > 0 {
                    return true;
                }
            } else if piece.is_newline() {
                newlines += piece.byte_len().max(1);
                if matches!(piece, TriviaPiece::CarriageReturnLineFeeds(count) if *count < 2) {
                    newlines -= 1;
                }
            }
        }
        newlines >= 2
    }

    /// A blank line separates the item from the one before it, possibly with
    /// comments in between.
    fn has_blank_line_before(item: &SyntaxNode) -> bool {
        let mut newlines = 0;
        for piece in item.first_token().leading_trivia().iter() {
            match piece {
                TriviaPiece::LineComment(_) | TriviaPiece::BlockComment(_) => newlines = 0,
                TriviaPiece::CarriageReturnLineFeeds(count) => newlines += count,
                _ if piece.is_newline() => newlines += piece.byte_len(),
                _ => {}
            }
            // The first newline ends the line of the previous item.
            if newlines >= 2 {
                return true;
            }
        }
        false
    }

    fn has_blank_line_above(item: &SyntaxNode) -> bool {
        let mut newlines = 0;
        for piece in item.first_token().leading_trivia().iter() {
            match piece {
                TriviaPiece::LineComment(_) | TriviaPiece::BlockComment(_) => newlines = 0,
                TriviaPiece::CarriageReturnLineFeeds(count) => newlines += count,
                _ if piece.is_newline() => newlines += piece.byte_len(),
                _ => {}
            }
            if newlines >= 2 {
                return true;
            }
        }
        false
    }

    fn first_token_of(node: &SyntaxNode, kind: NodeKind) -> Option<usize> {
        node.children()
            .find(|child| child.kind() == kind)
            .map(|child| child.first_token().text_offset())
    }

    fn direct_token(node: &SyntaxNode, accept: impl Fn(TokenKind) -> bool) -> Option<usize> {
        node.tokens()
            .find(|token| accept(token.kind()))
            .map(|token| token.text_offset())
    }

    /// The columns of a declaration: its colon and, if it has one, `:=`.
    ///
    /// Without a space before the colon, the column starts after it.
    fn declaration_targets(&self, item: &SyntaxNode) -> Vec<(usize, usize)> {
        let colon = item
            .tokens()
            .find(|token| token.kind() == TokenKind::Colon)
            .and_then(|colon| {
                if self.config.space_before_colon {
                    Some(colon)
                } else {
                    colon.next_token()
                }
            })
            .map(|token| token.text_offset());
        let value = Self::first_token_of(item, NodeKind::InitialValue);
        colon
            .map(|colon| (colon, 1))
            .into_iter()
            .chain(value.map(|value| (value, 0)))
            .collect()
    }

    /// Columns of a declaration or statement in a block, if the configuration
    /// asks for them: the colon and `:=` of object declarations and record
    /// elements, and the operator of unlabeled assignments.
    fn block_row_targets(&self, item: &SyntaxNode) -> Vec<(usize, usize)> {
        use NodeKind::*;
        match item.kind() {
            ConstantDeclaration | SignalDeclaration | VariableDeclaration | FileDeclaration
            | ElementDeclaration
                if self.config.align_declarations =>
            {
                self.declaration_targets(item)
            }
            AttributeDeclaration if self.config.align_declarations => {
                self.declaration_targets(item)
            }
            // The colon is the entity specification's; `is` is a second column.
            AttributeSpecification if self.config.align_declarations => {
                let mut targets = item
                    .children()
                    .find(|child| child.kind() == EntitySpecification)
                    .map(|entity| self.declaration_targets(&entity))
                    .unwrap_or_default();
                targets.extend(
                    Self::direct_token(item, |kind| kind == TokenKind::Keyword(Keyword::Is))
                        .map(|is| (is, 1)),
                );
                targets
            }
            SimpleVariableAssignment
            | ConditionalVariableAssignment
            | SimpleWaveformAssignment
            | ConditionalWaveformAssignment
            | ConcurrentSimpleSignalAssignment
            | ConcurrentConditionalSignalAssignment
                if self.config.align_assignments =>
            {
                let plain = item.children().all(|child| child.kind() != StmtLabel)
                    && item.tokens().all(|token| {
                        !matches!(
                            token.kind(),
                            TokenKind::Keyword(Keyword::Guarded | Keyword::Postponed)
                        )
                    });
                Self::direct_token(item, |kind| {
                    matches!(kind, TokenKind::ColonEq | TokenKind::LTE)
                })
                .filter(|_| plain)
                .map(|operator| (operator, 1))
                .into_iter()
                .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Columns of an entry of a parenthesized list, if the configuration asks
    /// for them. `expanded` lists always have one entry per line.
    fn list_row_targets(
        &self,
        list: &SyntaxNode,
        item: &SyntaxNode,
        expanded: bool,
    ) -> Vec<(usize, usize)> {
        use NodeKind::*;
        let mode = |item: &SyntaxNode| {
            item.tokens().any(|token| {
                matches!(
                    token.kind(),
                    TokenKind::Keyword(
                        Keyword::In
                            | Keyword::Out
                            | Keyword::Inout
                            | Keyword::Buffer
                            | Keyword::Linkage
                    )
                )
            })
        };
        match item.kind() {
            InterfaceObjectDeclaration if self.config.align_declarations => {
                let mut targets = self.declaration_targets(item);
                // Types line up after the modes only if every entry has one.
                let all_modes = list
                    .children()
                    .all(|other| other.kind() == InterfaceObjectDeclaration && mode(&other));
                if let Some(subtype) =
                    Self::first_token_of(item, SubtypeIndication).filter(|_| all_modes)
                {
                    if !targets.iter().any(|(target, _)| *target == subtype) {
                        targets.insert(1.min(targets.len()), (subtype, 0));
                    }
                }
                targets
            }
            InterfaceFileDeclaration if self.config.align_declarations => {
                self.declaration_targets(item).into_iter().take(1).collect()
            }
            AssociationElement if self.config.align_associations => item
                .children()
                .find(|child| child.kind() == Formal)
                .and_then(|formal| {
                    Self::direct_token(&formal, |kind| kind == TokenKind::RightArrow)
                })
                .map(|arrow| (arrow, 1))
                .into_iter()
                .collect(),
            ElementAssociation if self.config.align_associations && expanded => item
                .children()
                .find(|child| child.kind() == ElementChoices)
                .and_then(|choices| {
                    Self::direct_token(&choices, |kind| kind == TokenKind::RightArrow)
                })
                .map(|arrow| (arrow, 1))
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Pad the rows of a run so that their columns line up, then start a new
    /// run. A row that is too long for one line still takes part if it can
    /// wrap after its last column; other rows that do not fit end the run.
    /// With `when_broken`, the padding only applies if the enclosing list is
    /// not on one line.
    fn align_rows(&mut self, run: &mut Vec<usize>, level: usize, when_broken: bool) {
        let available = self
            .config
            .max_width
            .saturating_sub(level * self.config.indent_width);
        let mut rows: Vec<AlignedRow> = Vec::new();
        let run = std::mem::take(run);
        for &index in &run {
            let doc = &self.docs[index];
            let row = doc.alignment_widths().map(|widths| AlignedRow {
                index,
                widths,
                flat: doc.flat_width(),
                head: doc.aligned_head_width(),
            });
            match row {
                Some(row) if row.fits(0, available) => rows.push(row),
                _ => {
                    self.align_run(&rows, available, when_broken);
                    rows.clear();
                }
            }
        }
        self.align_run(&rows, available, when_broken);
        for index in run {
            self.docs[index].settle_alignments();
        }
    }

    fn align_run(&mut self, rows: &[AlignedRow], available: usize, when_broken: bool) {
        if rows.len() < 2 {
            return;
        }
        // The last cell of a row is not followed by a column.
        let columns = rows
            .iter()
            .map(|row| row.widths.len() - 1)
            .max()
            .unwrap_or(0);
        let widest: Vec<usize> = (0..columns)
            .map(|column| {
                rows.iter()
                    .filter(|row| row.widths.len() > column + 1)
                    .map(|row| row.widths[column])
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let padding = |widths: &[usize]| -> Vec<usize> {
            widest
                .iter()
                .zip(&widths[..widths.len() - 1])
                .map(|(widest, width)| widest - width)
                .collect()
        };
        // A cell much wider than the others of its column would push them
        // all far to the right. Its row ends the run instead.
        let outlier = (0..columns).find_map(|column| {
            let cells =
                |row: &AlignedRow| (row.widths.len() > column + 1).then(|| row.widths[column]);
            let mut sorted: Vec<usize> = rows.iter().filter_map(cells).collect();
            sorted.sort_unstable();
            let median = sorted[(sorted.len() - 1) / 2];
            rows.iter()
                .position(|row| cells(row).is_some_and(|width| width > median + MAX_ALIGNMENT_GAP))
        });
        let too_wide = outlier.or_else(|| {
            rows.iter()
                .position(|row| !row.fits(padding(&row.widths).iter().sum(), available))
        });
        if let Some(too_wide) = too_wide {
            self.align_run(&rows[..too_wide], available, when_broken);
            self.align_run(&rows[too_wide + 1..], available, when_broken);
            return;
        }
        for row in rows {
            let padding = padding(&row.widths);
            if when_broken {
                self.docs[row.index].pad_alignments_when_broken(&padding);
            } else {
                self.docs[row.index].pad_alignments(&padding);
            }
        }
    }

    /// A declaration or statement. Each break inside it that no inner group
    /// owns is taken only if the text up to the next break does not fit.
    fn item_node(&mut self, node: &SyntaxNode, indent: usize, newlines: usize) -> Option<usize> {
        self.level += indent;
        self.break_line(newlines);
        let doc = if is_skipped(node) {
            self.verbatim(node);
            None
        } else {
            self.grouped(Shape::Fill, |emitter| emitter.node(node))
        };
        self.level -= indent;
        doc
    }

    /// Emit a node as it is written. Only its first line is indented anew.
    fn verbatim(&mut self, node: &SyntaxNode) {
        let mut tokens = node.visit_tokens();
        let Some(first) = tokens.next() else {
            return;
        };
        let mut text = decode(first.text().as_bytes());
        let mut last = first.clone();
        for token in tokens {
            let mut bytes = Vec::new();
            token
                .leading_trivia()
                .write_to(&mut bytes)
                .expect("writing to a vector succeeds");
            text.push_str(&decode(&bytes));
            text.push_str(&decode(token.text().as_bytes()));
            last = token;
        }
        self.token_as(&first, Some(text.replace("\r\n", "\n")));
        self.previous = Some(last);
    }

    /// Library clauses start a line; the use clauses and context references
    /// that follow a library clause are indented below it. Inside a context
    /// declaration everything is one level deeper.
    fn context_clause(&mut self, node: &SyntaxNode, parent: Option<NodeKind>) {
        let base = usize::from(parent == Some(NodeKind::ContextDeclaration));
        let mut below_library = 0;
        for child in node.children_with_tokens() {
            match child {
                // A library clause that follows other clauses starts a new
                // paragraph.
                SyntaxElement::Node(child) if child.kind() == NodeKind::LibraryClause => {
                    let newlines = if child.prev_sibling().is_some() { 2 } else { 1 };
                    below_library = 1;
                    self.item_node(&child, base, newlines);
                }
                SyntaxElement::Node(child) => {
                    self.item_node(&child, base + below_library, 1);
                }
                SyntaxElement::Token(token) => self.token(&token),
            }
        }
    }

    /// Number of entries that count towards `inline_argument_limit`, or `None`
    /// if the list is only limited by the line width.
    fn limited_count(list: &SyntaxNode) -> Option<usize> {
        use NodeKind::*;
        let items = list.children();
        match list.kind() {
            // Names declared together count individually.
            InterfaceList => Some(
                items
                    .map(|item| {
                        item.children()
                            .find(|child| child.kind() == IdentifierList)
                            .map_or(1, |names| {
                                names
                                    .tokens()
                                    .filter(|token| token.kind() == TokenKind::Identifier)
                                    .count()
                            })
                    })
                    .sum(),
            ),
            AssociationList => Some(items.count()),
            // A purely positional aggregate may fill the line.
            ElementAssociationList => {
                let items: Vec<_> = items.collect();
                items
                    .iter()
                    .any(|item| item.children().any(|child| child.kind() == ElementChoices))
                    .then_some(items.len())
            }
            _ => None,
        }
    }

    /// A parenthesized, separated list. It stays on one line if it fits and,
    /// where `limited`, has at most `inline_argument_limit` entries. Otherwise
    /// every entry gets a line, one level below the line the list starts on.
    fn delimited(&mut self, node: &SyntaxNode, limited: bool) {
        let mut opened = None;
        let outer = self.level;
        let mut base = outer;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Token(token) if token.kind() == TokenKind::LeftPar => {
                    opened = Some(self.open_group());
                    self.token(&token);
                    base = self.line_start_level.unwrap_or(outer);
                    self.level = base + 1;
                    self.pending = Some(Break::Soft { space: false });
                }
                SyntaxElement::Token(token) if token.kind() == TokenKind::RightPar => {
                    self.level = base;
                    if opened.is_some() {
                        self.pending = Some(Break::Soft { space: false });
                    }
                    self.token(&token);
                    if let Some(opened) = opened.take() {
                        self.close_group(opened, base, Shape::Anchored);
                    }
                    self.level = outer;
                }
                SyntaxElement::Token(token) => self.token(&token),
                SyntaxElement::Node(list) if opened.is_some() => {
                    let count = Self::limited_count(&list).filter(|_| limited);
                    let expanded =
                        count.is_some_and(|count| count > self.config.inline_argument_limit);
                    if expanded {
                        self.pending = Some(Break::Hard(1, Edge::Inside));
                    }
                    let mut run = Vec::new();
                    // Short positional values are packed onto as few lines
                    // as the width allows, not spread one per line.
                    let packed = list.kind() == NodeKind::ElementAssociationList
                        && count.is_none()
                        && list.children().all(|item| Self::is_scalar(&item));
                    let mut entries = |emitter: &mut Self| {
                        for item in list.children_with_tokens() {
                            match item {
                                SyntaxElement::Node(item) if packed => emitter.node(&item),
                                SyntaxElement::Node(item) => {
                                    let targets = emitter.list_row_targets(&list, &item, expanded);
                                    let is_row = !targets.is_empty();
                                    // Unlike in a block, a comment between
                                    // the entries does not end the run.
                                    if !is_row || Self::has_blank_line_before(&item) {
                                        emitter.align_rows(&mut run, base + 1, !expanded);
                                    }
                                    // Where columns are aligned, a blank line
                                    // separates the groups of rows.
                                    if is_row
                                        && emitter.has_content
                                        && Self::has_blank_line_above(&item)
                                        && matches!(
                                            emitter.pending,
                                            Some(Break::Soft { space: true })
                                        )
                                    {
                                        emitter.pending = Some(Break::Hard(2, Edge::Inside));
                                    }
                                    let row = emitter.with_targets(targets, |emitter| {
                                        emitter.grouped(Shape::Fill, |emitter| emitter.node(&item))
                                    });
                                    run.extend(row.filter(|_| is_row));
                                }
                                SyntaxElement::Token(token) => {
                                    emitter.token(&token);
                                    if matches!(
                                        token.kind(),
                                        TokenKind::Comma | TokenKind::SemiColon
                                    ) {
                                        emitter.pending = Some(Break::Soft { space: true });
                                    }
                                }
                            }
                        }
                    };
                    if packed {
                        self.grouped(Shape::Fill, entries);
                    } else {
                        entries(self);
                    }
                    self.align_rows(&mut run, base + 1, !expanded);
                }
                SyntaxElement::Node(child) => self.node(&child),
            }
        }
        // Only reached with an unbalanced list, which does not parse.
        if let Some(opened) = opened {
            self.close_group(opened, base, Shape::Anchored);
            self.level = outer;
        }
    }

    /// A literal or simple name, possibly signed: nothing that could itself
    /// need wrapping.
    fn is_scalar(item: &SyntaxNode) -> bool {
        item.descendants_with_tokens().all(|element| match element {
            SyntaxElement::Node(node) => node.kind() != NodeKind::BinaryExpression,
            SyntaxElement::Token(token) => {
                !matches!(token.kind(), TokenKind::LeftPar | TokenKind::Dot)
            }
        })
    }

    /// Entries separated by `,` or `|` that are direct children of `node`.
    /// When the group breaks, every entry after the first starts a line.
    fn separated(&mut self, node: &SyntaxNode) {
        // Alternatives continue one level deeper, and so do names after a
        // keyword such as `signal`. Other entries line up with the first.
        let deeper = usize::from(match node.kind() {
            NodeKind::Choices => true,
            NodeKind::IdentifierList => node.prev_sibling_or_token().is_some(),
            _ => false,
        });
        let mut entered = 0;
        for child in node.children_with_tokens() {
            match child {
                // Breaks inside an entry are decided for that entry alone.
                SyntaxElement::Node(child) => {
                    self.grouped(Shape::Fill, |emitter| emitter.node(&child));
                }
                SyntaxElement::Token(token) if token.kind() == TokenKind::Bar => {
                    self.level += deeper;
                    self.soft_break(true);
                    self.token(&token);
                    self.level -= deeper;
                }
                SyntaxElement::Token(token) => {
                    self.token(&token);
                    if token.kind() == TokenKind::Comma {
                        if entered == 0 {
                            self.level += deeper;
                            entered = deeper;
                        }
                        self.soft_break(true);
                    }
                }
            }
        }
        self.level -= entered;
    }

    /// Everything after the first token accepted by `is_operator` may move to
    /// a new line, one level deeper.
    fn after_token(&mut self, node: &SyntaxNode, is_operator: impl Fn(TokenKind) -> bool) {
        let children: Vec<_> = node.children_with_tokens().collect();
        self.after_token_in(&children, is_operator);
    }

    fn after_token_in(
        &mut self,
        children: &[SyntaxElement],
        is_operator: impl Fn(TokenKind) -> bool,
    ) {
        let outer = self.level;
        for child in children {
            match child {
                // A clause such as `severity` is indented below the statement,
                // not below the text that was moved.
                SyntaxElement::Node(child) if breaks_before(child.kind()) => {
                    let level = std::mem::replace(&mut self.level, outer);
                    self.node(child);
                    self.level = level;
                }
                SyntaxElement::Node(child) => {
                    // A waveform with delays or several elements is one list:
                    // if it breaks, it starts on the line after the operator.
                    if child.kind() == NodeKind::WaveformElements
                        && matches!(self.pending, Some(Break::Soft { .. }))
                        && (child.children().count() > 1
                            || child
                                .descendants()
                                .any(|node| node.kind() == NodeKind::AfterClause))
                    {
                        self.break_inside_group = self.deferred_groups + 1;
                    }
                    self.node(child)
                }
                SyntaxElement::Token(token) => {
                    // A statement that ends right after its operator has
                    // nothing to move.
                    if token.kind() == TokenKind::SemiColon
                        && matches!(self.pending, Some(Break::Soft { .. }))
                    {
                        self.pending = None;
                    }
                    self.token(token);
                    if self.level == outer && is_operator(token.kind()) {
                        self.level += 1;
                        self.soft_break(true);
                    }
                }
            }
        }
        self.level = outer;
    }

    /// A break is possible after every token accepted by `is_break`, also
    /// inside child nodes one level down. What follows is one level deeper.
    fn after_each(&mut self, node: &SyntaxNode, is_break: impl Fn(TokenKind) -> bool + Copy) {
        let outer = self.level;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(child) if child.kind() == NodeKind::AliasSubtype => {
                    for part in child.children_with_tokens() {
                        match part {
                            SyntaxElement::Node(part) => self.node(&part),
                            SyntaxElement::Token(token) => {
                                self.token(&token);
                                if is_break(token.kind()) {
                                    self.level = outer + 1;
                                    self.soft_break(true);
                                }
                            }
                        }
                    }
                }
                SyntaxElement::Node(child) => self.node(&child),
                SyntaxElement::Token(token) => {
                    self.token(&token);
                    if is_break(token.kind()) {
                        self.level = outer + 1;
                        self.soft_break(true);
                    }
                }
            }
        }
        self.level = outer;
    }

    /// `<< class path : subtype >>`. If it does not fit, the delimiters get
    /// lines of their own, and the subtype may move below the path.
    fn external_name(&mut self, node: &SyntaxNode) {
        let outer = self.level;
        let mut subtype = None;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(child) => self.node(&child),
                SyntaxElement::Token(token) => {
                    if token.kind() == TokenKind::GtGt {
                        if let Some(opened) = subtype.take() {
                            self.close_group(opened, outer + 2, Shape::Together);
                        }
                        self.level = outer;
                        self.soft_break(true);
                    }
                    self.token(&token);
                    match token.kind() {
                        TokenKind::LtLt => {
                            self.level = outer + 1;
                            self.soft_break(true);
                        }
                        TokenKind::Colon if subtype.is_none() => {
                            self.level = outer + 2;
                            self.break_inside_group = 0;
                            self.pending = Some(Break::Preferred);
                            subtype = Some(self.open_group());
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(opened) = subtype {
            self.close_group(opened, outer + 2, Shape::Together);
        }
        self.level = outer;
    }

    /// An object, element or interface declaration. If it does not fit, the
    /// mode and type move to the next line before the type itself is broken.
    fn declaration(&mut self, node: &SyntaxNode) {
        let outer = self.level;
        let mut subtype = None;
        for child in node.children_with_tokens() {
            let ends_subtype = match &child {
                SyntaxElement::Node(child) => {
                    matches!(
                        child.kind(),
                        NodeKind::InitialValue | NodeKind::FileOpenInformation
                    )
                }
                SyntaxElement::Token(token) => token.kind() == TokenKind::SemiColon,
            };
            if ends_subtype {
                if let Some(opened) = subtype.take() {
                    self.close_group(opened, outer + 1, Shape::Together);
                }
                self.level = outer;
            }
            match child {
                SyntaxElement::Node(child) => self.node(&child),
                SyntaxElement::Token(token) => {
                    self.token(&token);
                    if token.kind() == TokenKind::Colon && subtype.is_none() && !ends_subtype {
                        self.level = outer + 1;
                        self.break_inside_group = 0;
                        self.pending = Some(Break::Preferred);
                        subtype = Some(self.open_group());
                    }
                }
            }
        }
        if let Some(opened) = subtype {
            self.close_group(opened, outer + 1, Shape::Together);
        }
        self.level = outer;
    }

    /// `value when condition` of a conditional assignment. The condition moves
    /// to a new line, one level deeper, before it is broken itself.
    fn conditional_item(&mut self, node: &SyntaxNode) {
        let outer = self.level;
        let mut condition = None;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(child) => self.node(&child),
                SyntaxElement::Token(token) => {
                    if token.kind() == TokenKind::Keyword(Keyword::When) && condition.is_none() {
                        self.level = outer + 1;
                        self.break_inside_group = 0;
                        self.pending = Some(Break::Preferred);
                        condition = Some(self.open_group());
                    }
                    self.token(&token);
                }
            }
        }
        if let Some(opened) = condition {
            self.close_group(opened, outer + 1, Shape::Together);
        }
        self.level = outer;
    }

    /// A selected assignment with more alternatives than fit a line by
    /// `inline_argument_limit`: every alternative gets a line, one level
    /// below the statement, and `align_assignments` lines up their `when`.
    fn expanded_selection(&mut self, children: &[SyntaxElement]) {
        let outer = self.level;
        self.grouped(Shape::Fill, |emitter| {
            for child in children {
                match child {
                    SyntaxElement::Node(list)
                        if matches!(
                            list.kind(),
                            NodeKind::SelectedWaveforms | NodeKind::SelectedExpressions
                        ) =>
                    {
                        emitter.level = outer + 1;
                        emitter.pending = Some(Break::Hard(1, Edge::First));
                        let mut run = Vec::new();
                        for item in list.children_with_tokens() {
                            match item {
                                SyntaxElement::Node(item) => {
                                    let targets = Self::direct_token(&item, |kind| {
                                        kind == TokenKind::Keyword(Keyword::When)
                                    })
                                    .filter(|_| emitter.config.align_assignments)
                                    .map(|when| (when, 1))
                                    .into_iter()
                                    .collect();
                                    let row = emitter.with_targets(targets, |emitter| {
                                        emitter.grouped(Shape::Fill, |emitter| emitter.node(&item))
                                    });
                                    run.extend(row);
                                }
                                SyntaxElement::Token(token) => {
                                    emitter.token(&token);
                                    emitter.pending = Some(Break::Hard(1, Edge::Inside));
                                }
                            }
                        }
                        emitter.align_rows(&mut run, outer + 1, false);
                        emitter.level = outer;
                    }
                    SyntaxElement::Node(child) => {
                        let is_preamble = child.kind() == NodeKind::SelectedAssignmentPreamble;
                        emitter.node(child);
                        if is_preamble {
                            emitter.level = outer + 1;
                            emitter.soft_break(true);
                        }
                    }
                    SyntaxElement::Token(token) => {
                        emitter.token(token);
                        emitter.level = outer;
                    }
                }
            }
        });
        self.level = outer;
    }

    /// `with ... select` stays on the first line. If the statement does not
    /// fit, the assignment moves to the next line, one level deeper.
    fn selected_assignment(&mut self, node: &SyntaxNode) {
        let children: Vec<_> = node.children_with_tokens().collect();
        let preamble = children
            .iter()
            .position(|child| {
                matches!(child, SyntaxElement::Node(child)
                    if child.kind() == NodeKind::SelectedAssignmentPreamble)
            })
            .map_or(0, |index| index + 1);
        let alternatives = children
            .iter()
            .filter_map(|child| match child {
                SyntaxElement::Node(child)
                    if matches!(
                        child.kind(),
                        NodeKind::SelectedWaveforms | NodeKind::SelectedExpressions
                    ) =>
                {
                    Some(child.children().count())
                }
                _ => None,
            })
            .next()
            .unwrap_or(0);
        if alternatives > self.config.inline_argument_limit {
            return self.expanded_selection(&children);
        }
        self.grouped(Shape::Together, |emitter| {
            for child in &children[..preamble] {
                match child {
                    SyntaxElement::Node(child) => emitter.node(child),
                    SyntaxElement::Token(token) => emitter.token(token),
                }
            }
            emitter.level += usize::from(preamble > 0);
            if preamble > 0 {
                emitter.soft_break(true);
            }
            emitter.grouped(Shape::Fill, |emitter| {
                emitter.after_token_in(&children[preamble..], |kind| {
                    matches!(kind, TokenKind::ColonEq | TokenKind::LTE)
                })
            });
            emitter.level -= usize::from(preamble > 0);
        });
    }

    /// `keyword` and what follows it may move to a new line, one level deeper.
    fn before_keyword(&mut self, node: &SyntaxNode, keyword: Keyword) {
        let outer = self.level;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(child) => self.node(&child),
                SyntaxElement::Token(token) => {
                    if token.kind() == TokenKind::Keyword(keyword) && self.level == outer {
                        self.level += 1;
                        self.soft_break(true);
                    }
                    self.token(&token);
                }
            }
        }
        self.level = outer;
    }

    /// The header of a compound statement, up to and including `end`. If it
    /// does not fit on one line, `end` starts a line at the statement's level.
    fn header(&mut self, node: &SyntaxNode, end: Keyword) {
        let mut opened = Some(self.open_group());
        let level = self.level;
        let mut first = true;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(child) => self.node(&child),
                SyntaxElement::Token(token) => {
                    let is_end = token.kind() == TokenKind::Keyword(end) && opened.is_some();
                    if is_end && !first {
                        self.soft_break(true);
                    }
                    self.token(&token);
                    if is_end {
                        self.close_group(
                            opened.take().expect("open header"),
                            level,
                            Shape::Anchored,
                        );
                    }
                }
            }
            first = false;
        }
        if let Some(opened) = opened {
            self.close_group(opened, level, Shape::Anchored);
        }
    }

    fn binary_class(node: &SyntaxNode) -> u8 {
        node.tokens()
            .next()
            .map_or(0, |operator| operator_class(operator.kind()))
    }

    /// Operands of a chain of same-class operators, with a possible break
    /// before each operator, one level below the first operand.
    fn binary_chain(&mut self, node: &SyntaxNode, class: u8) {
        let mut first = true;
        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(operand) if first => {
                    first = false;
                    if operand.kind() == NodeKind::BinaryExpression
                        && class != 0
                        && Self::binary_class(&operand) == class
                    {
                        self.binary_chain(&operand, class);
                    } else {
                        self.node(&operand);
                    }
                }
                SyntaxElement::Node(operand) => {
                    self.level += 1;
                    self.node(&operand);
                    self.level -= 1;
                }
                SyntaxElement::Token(operator) => {
                    first = false;
                    self.level += 1;
                    // A range is never split at `to` or `downto`.
                    if class != 0 {
                        self.soft_break(true);
                    }
                    self.token(&operator);
                    self.level -= 1;
                }
            }
        }
    }

    /// Emit `build` as one group.
    /// Returns where in `docs` the group is, unless nothing was emitted.
    fn grouped(&mut self, shape: Shape, build: impl FnOnce(&mut Self)) -> Option<usize> {
        let opened = self.open_group();
        let level = self.level;
        build(self);
        self.close_group(opened, level, shape)
    }

    /// Request a group that starts at the next token. Returns a handle for
    /// [`Self::close_group`].
    fn open_group(&mut self) -> usize {
        self.deferred_groups += 1;
        self.group_starts.len() + self.deferred_groups
    }

    /// Close a group that was opened at indentation `level`.
    fn close_group(&mut self, opened: usize, level: usize, shape: Shape) -> Option<usize> {
        // Inside `-- fmt: off` no token is emitted, so the group never opened.
        if self.deferred_groups > 0 && self.group_starts.len() < opened {
            self.deferred_groups -= 1;
            return None;
        }
        let start = self.group_starts.pop().expect("balanced groups");
        let children = Doc::concat(self.docs.split_off(start));
        self.docs.push(Doc::indent(
            level,
            match shape {
                Shape::Fill => Doc::fill_group(children),
                Shape::Together => Doc::group(children),
                Shape::Anchored => Doc::expression_group(children),
            },
        ));
        Some(self.docs.len() - 1)
    }

    /// Request a new line before the next token; `newlines` is 2 for a blank
    /// line.
    fn break_line(&mut self, newlines: usize) {
        self.break_line_at(newlines, Edge::Inside);
    }

    fn break_line_at(&mut self, newlines: usize, edge: Edge) {
        let (newlines, edge) = match self.pending {
            Some(Break::Hard(pending, first)) => (
                pending.max(newlines),
                if edge == Edge::Inside { first } else { edge },
            ),
            _ => (newlines, edge),
        };
        self.pending = Some(Break::Hard(newlines, edge));
    }

    /// Allow a line break before the next token.
    fn soft_break(&mut self, space: bool) {
        self.break_inside_group = 0;
        if !matches!(self.pending, Some(Break::Hard(..))) {
            self.pending = Some(Break::Soft { space });
        }
    }

    fn token(&mut self, token: &SyntaxToken) {
        self.token_as(token, None);
    }

    /// Emit a token with its comments. `verbatim` replaces the token's text.
    fn token_as(&mut self, token: &SyntaxToken, verbatim: Option<String>) {
        let mut forced = self.pending.take().map(|kind| Forced { kind, done: false });
        let trivia = token.leading_trivia();
        let off = |piece: &TriviaPiece| {
            comment_text(piece).is_some_and(|text| directive(&text) == Some("fmt: off"))
        };
        // The indentation of the token's own line, if it starts one. Comments
        // above the token are compared with it. A token after `-- fmt: off`
        // keeps its source indentation, which says nothing about them.
        let token_columns = trivia
            .iter()
            .rposition(|piece| piece.is_newline())
            .filter(|&newline| trivia.iter().skip(newline).all(|piece| !piece.is_comment()))
            .filter(|_| !trivia.iter().any(off))
            .map(|newline| {
                let mut gap = Gap::default();
                for piece in trivia.iter().skip(newline + 1) {
                    self.whitespace(piece, &mut gap);
                }
                gap.columns
            });
        let mut gap = Gap::default();
        for (index, piece) in trivia.iter().enumerate() {
            let closes_line = trivia
                .iter()
                .skip(index + 1)
                .find(|next| !next.is_space_or_tab())
                .is_some_and(|next| next.is_newline());
            self.trivia(piece, &mut gap, &mut forced, token_columns, closes_line);
        }
        if token.kind() == TokenKind::Eof {
            return;
        }
        let text = decode(token.text().as_bytes());
        if let Some(raw) = &mut self.raw {
            raw.push_str(&text);
        } else {
            let space = self.after_comment
                || self.previous.as_ref().is_some_and(|previous| {
                    space_between(
                        self.before_previous,
                        previous.kind(),
                        self.previous_is_sign,
                        token.kind(),
                    ) || requires_separator(previous.token(), token.token(), self.standard)
                })
                || (token.kind() == TokenKind::Colon
                    && (self.spaced_colon > 0
                        || (self.config.space_before_colon
                            && is_declaration_colon(token.parent().kind()))));
            let cased = match token.kind() {
                TokenKind::Keyword(_) => true,
                TokenKind::Identifier => {
                    self.previous
                        .as_ref()
                        .is_some_and(|previous| previous.kind() == TokenKind::Tick)
                        && is_predefined_attribute(token.text().as_bytes())
                }
                _ => false,
            };
            let text = match (verbatim, cased, self.config.keyword_case) {
                (Some(verbatim), ..) => verbatim,
                (None, true, KeywordCase::Lower) => text.to_ascii_lowercase(),
                (None, true, KeywordCase::Upper) => text.to_ascii_uppercase(),
                (None, false, _) => text,
            };
            let offset = token.text_offset();
            self.align = self
                .align_targets
                .iter()
                .find(|(target, _)| *target == offset)
                .map(|(_, spaces)| *spaces);
            self.item(&mut gap, space, text, &mut forced, None);
            self.align = None;
        }
        let previous = self.previous.replace(token.clone());
        self.before_previous = previous.as_ref().map(SyntaxToken::kind);
        self.previous_is_sign = matches!(token.kind(), TokenKind::Plus | TokenKind::Minus)
            && !previous.is_some_and(|previous| ends_operand(previous.kind()));
        self.after_comment = false;
        self.after_closing_comment = false;
    }

    fn whitespace(&self, piece: &TriviaPiece, gap: &mut Gap) {
        use TriviaPiece::*;
        match piece {
            Spaces(count) | NonBreakingSpaces(count) => gap.columns += count,
            HorizontalTabs(count) => gap.columns += count * self.config.indent_width,
            LineFeeds(count)
            | CarriageReturns(count)
            | CarriageReturnLineFeeds(count)
            | FormFeeds(count)
            | VerticalTabs(count) => {
                gap.newlines += count;
                gap.columns = 0;
            }
            LineComment(_) | BlockComment(_) => {}
        }
    }

    fn trivia(
        &mut self,
        piece: &TriviaPiece,
        gap: &mut Gap,
        forced: &mut Option<Forced>,
        token_columns: Option<usize>,
        closes_line: bool,
    ) {
        let comment = comment_text(piece);
        if let Some(raw) = &mut self.raw {
            match &comment {
                Some(comment) if directive(comment) == Some("fmt: on") => {
                    raw.push_str(comment);
                    let raw = self.raw.take().expect("raw region");
                    self.docs.push(Doc::text(raw));
                    self.after_comment = true;
                    self.after_closing_comment = true;
                    *gap = Gap::default();
                }
                _ => {
                    let mut bytes = Vec::new();
                    piece
                        .write_to(&mut bytes)
                        .expect("writing to a vector succeeds");
                    raw.push_str(&decode(&bytes));
                }
            }
            return;
        }
        let Some(comment) = comment else {
            self.whitespace(piece, gap);
            return;
        };
        let is_line = matches!(piece, TriviaPiece::LineComment(_));
        let off = is_line && directive(&comment) == Some("fmt: off");
        // A comment indented deeper than the token below it belongs to the
        // block that the token closes, for example before `end`.
        let deeper = token_columns.is_some_and(|columns| gap.columns > columns);
        self.item(gap, true, comment, forced, Some(deeper));
        self.after_comment = true;
        self.after_closing_comment = is_line || closes_line;
        if off {
            self.raw = Some(String::new());
        }
    }

    /// Emit a token, or a comment (`deeper_comment` is then set), after the
    /// whitespace described by `gap`.
    fn item(
        &mut self,
        gap: &mut Gap,
        space: bool,
        text: String,
        forced: &mut Option<Forced>,
        deeper_comment: Option<bool>,
    ) {
        let gap = std::mem::take(gap);
        let is_comment = deeper_comment.is_some();
        // Between declarations and statements the source decides about blank
        // lines. Elsewhere only a comment on its own line keeps its break.
        let edge = forced.as_ref().and_then(|forced| match forced.kind {
            Break::Hard(_, edge) => Some((edge, forced.done)),
            _ => None,
        });
        let source_newlines = match (edge, is_comment) {
            // No blank line opens a block, or sits directly above its end.
            (Some((Edge::First, false)), _) | (Some((Edge::Closing, _)), false) => {
                gap.newlines.min(1)
            }
            (Some(_), _) => gap.newlines,
            (None, true) => gap.newlines.min(1),
            (None, false) => 0,
        };
        let mut newlines = source_newlines.max(usize::from(self.after_closing_comment));
        // A comment on the line of the previous token stays there; the
        // requested break follows it.
        let trails_previous = is_comment && newlines == 0 && self.has_content;
        let mut soft = None;
        if let Some(forced) = forced.as_mut().filter(|_| !trails_previous) {
            if !forced.done {
                match forced.kind {
                    Break::Hard(count, _) => newlines = newlines.max(count),
                    // After a comment on the same line, a space is needed.
                    Break::Soft { space } => {
                        soft = Some(Doc::soft_line(space || self.after_comment))
                    }
                    Break::Preferred => soft = Some(Doc::preferred_line()),
                }
                forced.done = true;
            }
        }
        self.line_start_level = None;
        if !self.has_content || newlines > 0 {
            if !is_comment {
                self.break_inside_group = 0;
            }
            if self.has_content {
                // At most one blank line survives.
                for _ in 0..newlines.min(2) {
                    self.docs.push(Doc::hard_line());
                }
            }
            let level = self.level + usize::from(deeper_comment == Some(true));
            self.open_deferred_groups(is_comment);
            self.docs.push(Doc::text_at(level, text));
            if !is_comment {
                self.line_start_level = Some(level);
            }
        } else {
            // A column that brings its own space replaces the usual one.
            let align = self.align.filter(|_| !is_comment);
            // A break that belongs to the group starting here goes inside it.
            if !is_comment {
                for _ in 0..std::mem::take(&mut self.break_inside_group) {
                    if self.deferred_groups > 0 {
                        self.deferred_groups -= 1;
                        self.group_starts.push(self.docs.len());
                    }
                }
            }
            let soft_taken = soft.is_some();
            // The gap before a comment that ends a line of code is decided
            // once the lines are known.
            let ends_code_line = is_comment && text.starts_with("--");
            match soft {
                Some(soft) => self.docs.push(soft),
                None if ends_code_line => self.docs.push(Doc::text(COMMENT_GAP.to_string())),
                None if space && align.is_none_or(|spaces| spaces == 0) => {
                    self.docs.push(Doc::space())
                }
                None => {}
            }
            if let Some(spaces) = align {
                // A break before the column already is a space when not taken.
                let spaces = if soft_taken { 0 } else { spaces };
                self.docs.push(Doc::align(spaces));
            }
            self.open_deferred_groups(is_comment);
            self.docs.push(Doc::text_at(self.level, text));
        }
        self.has_content = true;
    }

    fn open_deferred_groups(&mut self, is_comment: bool) {
        if !is_comment {
            for _ in 0..std::mem::take(&mut self.deferred_groups) {
                self.group_starts.push(self.docs.len());
            }
        }
    }

    /// Render the document, ending in exactly one line break.
    pub(crate) fn finish(mut self) -> String {
        if let Some(raw) = self.raw.take() {
            self.docs.push(Doc::text(raw));
        }
        let rendered =
            layout::render_docs(&self.docs, self.config.indent_width, self.config.max_width);
        let rendered = rendered.trim_end().replace("\r\n", "\n");
        let mut output = self.place_comments(&rendered);
        if !output.is_empty() {
            output.push('\n');
        }
        output
    }

    /// Put one space before each comment that ends a line of code or, with
    /// `align_comments`, line such comments up on neighbouring lines of the
    /// same indentation. A run is left alone if lining it up would push a
    /// line over the width.
    fn place_comments(&self, rendered: &str) -> String {
        let width = |text: &str| text.chars().count();
        let split = |line: &str| -> Option<(String, String)> {
            line.split_once(COMMENT_GAP)
                .map(|(code, comment)| (code.to_owned(), comment.to_owned()))
        };
        let indent = |line: &str| line.len() - line.trim_start_matches(' ').len();
        let lines: Vec<&str> = rendered.split('\n').collect();
        let mut output = String::with_capacity(rendered.len());
        let mut index = 0;
        while index < lines.len() {
            let Some(first) = split(lines[index]) else {
                output.push_str(lines[index]);
                output.push('\n');
                index += 1;
                continue;
            };
            let mut run = vec![first];
            // A line that closes a list is not a row of the lines around it.
            let closes = |line: &str| line.trim_start().starts_with(')');
            while self.config.align_comments
                && !closes(lines[index])
                && index + run.len() < lines.len()
                && !closes(lines[index + run.len()])
                && indent(lines[index + run.len()]) == indent(lines[index])
            {
                match split(lines[index + run.len()]) {
                    Some(row) => run.push(row),
                    None => break,
                }
            }
            let column = run.iter().map(|(code, _)| width(code)).max().unwrap_or(0);
            let fits = |code: &str, comment: &str, gap: usize| {
                width(code) + gap + width(comment) <= self.config.max_width
            };
            let aligned = run.iter().all(|(code, comment)| {
                fits(code, comment, column - width(code) + 1) || !fits(code, comment, 1)
            });
            for (code, comment) in &run {
                let gap = if aligned { column - width(code) + 1 } else { 1 };
                output.push_str(code);
                output.extend(std::iter::repeat_n(' ', gap));
                output.push_str(comment);
                output.push('\n');
            }
            index += run.len();
        }
        output.pop();
        output
    }
}
