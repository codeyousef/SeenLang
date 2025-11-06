# Seen Language — Grammar Specification

The Seen grammar is expressed in EBNF using canonical keywords. Word-operators (`and`, `or`, `not`) behave as infix tokens with explicit precedence as listed below.

## 1. Operator Precedence (Low → High)
| Level | Operators / Forms | Associativity |
| --- | --- | --- |
| 1 | assignment `= += -= *= /=` | right |
| 2 | `or` | left |
| 3 | `and` | left |
| 4 | `== !=` | left |
| 5 | `< > <= >= is` | left |
| 6 | `+ -` | left |
| 7 | `* / %` | left |
| 8 | unary `not -` | right |
| 9 | postfix call `()`, index `[]`, member `.`, safe-call `?.` | left |

## 2. Compilation Units
```
CompilationUnit  = { ImportDecl } { TopLevelDecl } EOF ;
ImportDecl       = 'import' QualifiedIdent [ 'as' Identifier ] ';' ;
QualifiedIdent   = Identifier { '.' Identifier } ;
TopLevelDecl     = FunDecl
                 | StructDecl
                 | EnumDecl
                 | ExtensionDecl
                 | TraitDecl
                 | ConstDecl ;
```

## 3. Declarations
```
ConstDecl    = (KW_LET | KW_VAR | 'pub' KW_LET | 'pub' KW_VAR)
               Identifier [ ':' Type ] '=' Expression ';' ;

FunDecl      = Attributes? [ KW_ASYNC ] ( 'pub'? KW_FUN )
               Name [ GenericParams ]
               '(' ParamList? ')' [ ':' Type ]
               ( Block | '=>' Expression ) ;
Attributes   = '#[' Attribute (',' Attribute)* ']' ;
ParamList    = Param (',' Param)* ;
Param        = Pattern ':' Type [ '=' Expression ] ;
Name         = Identifier ;

StructDecl   = Attributes?
               ( 'pub'? KW_STRUCT ) Identifier [ GenericParams ]
               '{' FieldDecl* '}' ;
FieldDecl    = Attributes? Identifier ':' Type ';' ;

EnumDecl     = Attributes?
               ( 'pub'? KW_ENUM ) Identifier [ GenericParams ]
               '{' EnumVariant (',' EnumVariant)* ','? '}' ;
EnumVariant  = Identifier [ '(' Type (',' Type)* ')' ] [ '=' Expression ] ;

TraitDecl    = Attributes?
               ( 'pub'? KW_TRAIT ) Identifier [ GenericParams ]
               '{' TraitItem* '}' ;
TraitItem    = FunSignature ';' | AssociatedTypeDecl ';' ;
AssociatedTypeDecl = KW_TYPE Identifier [ ':' TypeBounds ] ;

ExtensionDecl = Attributes?
                KW_EXTENSION TypeName [ KW_FOR TraitBounds ] '{' ExtensionItem* '}' ;
ExtensionItem = FunDecl | ConstDecl ;
GenericParams = '<' GenericParam (',' GenericParam)* '>' ;
GenericParam  = Identifier [ ':' TypeBounds ] ;
TypeBounds    = Type ( '+' Type )* ;
```

## 4. Statements
```
Block        = '{' Statement* '}' ;
Statement    = ConstDecl
             | IfStmt
             | WhileStmt
             | ForStmt
             | ScopeStmt
             | MatchStmt
             | DeferStmt
             | ReturnStmt
             | BreakStmt
             | ContinueStmt
             | ExprStmt ;

ExprStmt     = Expression ';' ;
ReturnStmt   = KW_RETURN Expression? ';' ;
BreakStmt    = KW_BREAK ';' ;
ContinueStmt = KW_CONTINUE ';' ;

DeferStmt    = 'defer' ( Block | Expression ) ';'? ;
ScopeStmt    = KW_SCOPE '(' ScopeParam? ')' Block ;
ScopeParam   = Identifier '=>' ;

IfStmt       = KW_IF '(' Expression ')' Block ( KW_ELSE Block | KW_ELSE IfStmt )? ;
WhileStmt    = KW_WHILE '(' Expression ')' Block ;
ForStmt      = KW_FOR '(' Pattern KW_IN Expression ')' Block ;

MatchStmt    = MatchExpr ';' ;
```

## 5. Patterns
```
Pattern         = WildcardPattern
                | IdentifierPattern
                | TuplePattern
                | StructPattern
                | EnumPattern
                | LiteralPattern ;
WildcardPattern = '_' ;
IdentifierPattern = Identifier [ '@' Pattern ] ;
TuplePattern    = '(' Pattern (',' Pattern)* ','? ')' ;
StructPattern   = Path '{' FieldPatternList? '}' ;
FieldPatternList= FieldPattern (',' FieldPattern)* ','? ;
FieldPattern    = Identifier (':' Pattern)? ;
EnumPattern     = Path '(' Pattern (',' Pattern)* ','? ')' ;
LiteralPattern  = Literal ;
```

## 6. Types
```
Type            = TypeTerm '?'? ;
TypeTerm        = TypeName GenericArgs?
                | '(' Type (',' Type)* ','? ')' ;
TypeName        = QualifiedIdent | 'Self' ;
GenericArgs     = '<' Type (',' Type)* '>' ;
```

## 7. Expressions
```
Expression     = Assignment ;
Assignment     = LogicOr ( AssignOp LogicOr )? ;
AssignOp       = '=' | '+=' | '-=' | '*=' | '/=' ;

LogicOr        = LogicAnd ( KW_OR LogicAnd )* ;
LogicAnd       = Equality ( KW_AND Equality )* ;
Equality       = Relational ( ( '==' | '!=' ) Relational )* ;
Relational     = Additive ( ( '<' | '>' | '<=' | '>=' | KW_IS ) Additive )* ;
Additive       = Multiplicative ( ( '+' | '-' ) Multiplicative )* ;
Multiplicative = Unary ( ( '*' | '/' | '%' ) Unary )* ;

Unary          = ( KW_NOT | '-' | '&' | 'mut' KW_REF | KW_REF ) Unary
               | Postfix ;
Postfix        = Primary PostfixOp* ;
PostfixOp      = Call | Index | Member | SafeCall | Await ;
Call           = '(' ArgList? ')' ;
Index          = '[' Expression ']' ;
Member         = '.' Identifier ;
SafeCall       = '?.' Identifier Call? ;
Await          = KW_AWAIT ;
ArgList        = Expression (',' Expression)* ;

Primary        = Literal
               | Identifier
               | '(' Expression ')'
               | IfExpr
               | MatchExpr
               | Lambda
               | RegionExpr ;

Lambda         = '|' LambdaParams? '|' Expression
               | '|' LambdaParams? '|' Block ;
LambdaParams   = Param (',' Param)* ;

IfExpr         = KW_IF '(' Expression ')' Block KW_ELSE Block ;
MatchExpr      = KW_MATCH Expression '{' MatchArm+ '}' ;
MatchArm       = Pattern ( KW_IF Expression )? '->' Expression ';' ;

RegionExpr     = KW_REGION RegionHeader? Block ;
RegionHeader   = RegionStrategyHint Identifier?
               | Identifier ;
RegionStrategyHint = 'bump' | 'stack' | 'cxl_near' ;
```

## 8. Literals
```
Literal = IntegerLiteral
        | FloatLiteral
        | StringLiteral
        | ByteStringLiteral
        | CharLiteral
        | BoolLiteral
        | NullLiteral ;
BoolLiteral = KW_TRUE | KW_FALSE ;
NullLiteral = KW_NULL ;
```

## 9. Deterministic Formatting Constraints
- Formatter MUST respect precedence table in §1.
- Parenthesized expressions are preserved in the CST to avoid drift.
- Trivia captured by the lexer (§6 in `lexical.md`) is replayed deterministically; formatter rejects AST/precedence mismatches.
