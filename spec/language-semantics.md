
<p class="doc-nav"><a href="https://github.com/masamoto1982/Ajisai#readme">README</a> · <a href="docs/index.html">Reference</a> · <a href="https://masamoto1982.github.io/Ajisai/">Playground</a></p>

<h1 id="ajisai-language-semantics">Ajisai Language Semantics</h1>

<p>Status: <strong>Canonical</strong><br>
Specification version: <strong>1.0.0-beta.3</strong><br>
Release stage: <strong>Beta</strong>. A breaking change to the vocabulary, to program meaning, or to the host protocol raises the specification version and is named in <code>spec/CHANGELOG.md</code> and in the release that ships it; a compatibility guarantee begins at 1.0.0.</p>

<p>
This document defines the correspondence between Ajisai source programs and observable values, states, effects, and diagnoses. It is a compact semantic kernel: differences between individual Words belong to the machine-readable vocabulary registry, not to parallel prose definitions.
</p>

<p>
Ajisai is built from ten concepts. Everything below is one of them, or a consequence of one of them.
</p>

<ol>
<li>Exact arithmetic over fractions, closed under square roots and under division by zero, with no rounding.</li>
<li>Three outcomes: a value, a reasoned absence, or an error.</li>
<li>A stack of values and vectors of values.</li>
<li>Code blocks, evaluated only when a Word asks for it.</li>
<li>One consumption rule: a Word consumes what it reads, and <code>BIND</code> names a value for reuse.</li>
<li>A two-tier dictionary — sealed Core, user-defined User — with content-addressed identity.</li>
<li>A machine-readable contract for every Word.</li>
<li>A pre-execution check of user declarations against those contracts.</li>
<li>One host protocol, which is the only way anything outside the language observes it.</li>
<li>An executable conformance corpus that decides whether an implementation is Ajisai.</li>
</ol>

<nav class="toc">
<h2>Contents</h2>
<ol>
<li><a href="#lang-authority">Authority and Compatibility</a></li>
<li><a href="#lang-source">Source and Desugaring</a></li>
<li><a href="#lang-values">Value Domains</a></li>
<li><a href="#lang-machine">Machine State and Evaluation</a></li>
<li><a href="#lang-stack">Stack and Consumption</a></li>
<li><a href="#lang-failure">Partiality and Failure</a></li>
<li><a href="#lang-collections">Collections and Higher-order Evaluation</a></li>
<li><a href="#lang-dictionary">Dictionary and Effects</a></li>
<li><a href="#lang-contract">Contracts and Static Checking</a></li>
<li><a href="#lang-observation">Observation and Host Protocol</a></li>
<li><a href="#lang-conformance">Conformance</a></li>
</ol>
</nav>

<h2 id="lang-authority">1. Authority and Compatibility</h2>

<h3 id="lang-authority-sources">LANG.AUTHORITY.SOURCES — Normative sources</h3>

<p>Eight sources are authoritative, each for a different part of the language:</p>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Source</th><th>Authoritative for</th></tr></thead><tbody><tr><td>This Language Semantics</td><td>Program meaning</td></tr><tr><td><code>spec/grammar.json</code></td><td>The lexical grammar — the total correspondence from source text to a token sequence or one named source-error condition</td></tr><tr><td><code>spec/words.json</code></td><td>The vocabulary, and the laws Words share — a family's laws are the clauses every one of its Words cites</td></tr><tr><td><code>spec/identity.json</code></td><td>When two things are the same — the one law, and what each level that decides it can deliver</td></tr><tr><td><code>spec/termination.json</code></td><td>Why every evaluation is finite — the recursion sites, the measure each one decreases, and the invariant the argument rests on</td></tr><tr><td><code>spec/outcomes.json</code></td><td>The outcome space — the closed list of NIL reasons and ERROR categories a contract's names resolve into, including those no contract reaches (<code>literal</code>) and those belonging to no single Word (arity, source, dictionary, resource)</td></tr><tr><td><code>spec/gui-semantics.md</code></td><td>Presentation</td></tr><tr><td><code>spec/host-protocol.schema.json</code></td><td>The boundary between them</td></tr></tbody></table></div>

<p><code>SPECIFICATION.html</code> renders the two prose sources — this document and <code>spec/gui-semantics.md</code> — and is not edited directly. The six data sources are read directly by the implementation and its gates rather than through it: a source is authoritative because this table says so, not by appearing in that document.</p>

<p>
Neither implementation layout nor explanatory text can override an observable contract. A document that is not in the table above defines nothing; <code>docs/dev/</code> holds design notes and history on those terms. Within this document, a block marked <strong>Informative</strong> explains the clause it stands in and defines nothing either.
</p>

<h3 id="lang-authority-present">LANG.AUTHORITY.PRESENT — Present-tense description</h3>

<p>
Ajisai has three reading surfaces: the <strong>README</strong>, the <strong>Reference</strong>, and this <strong>Specification</strong>. Each describes the language as it currently is. A definition, an example, or an explanation may rest only on concepts the language has, and every Word it names must exist in the vocabulary registry.
</p>

<p>
Superseded designs, migration history, and the reasoning behind a change are recorded outside these three surfaces, in notes that define nothing. A negative statement belongs on a reading surface only when it constrains an implementation — "an implementation must not convert malformed use to NIL" is such a constraint — and not when its only content is a contrast with a design the language does not have.
</p>

<h3 id="lang-authority-identity">LANG.AUTHORITY.IDENTITY — Language identity</h3>

<p>
Ajisai identity is the correspondence from normalized source to the ordered observation of stack, output, dictionary state, and structured diagnosis. Two implementations are semantically equivalent when that correspondence agrees for every conforming program.
</p>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Count</th><th>What</th></tr></thead><tbody><tr><td>77</td><td>Canonical Words — the vocabulary (<code>docs/word-manifest.json</code> is the count of record)</td></tr><tr><td>49</td><td>Semantic Kernel Words, within the 77 — carry the language's semantic identity</td></tr><tr><td>28</td><td>Standard Words, within the 77 — carry its practical surface</td></tr></tbody></table></div>

<p>
Kernel and Standard are both ordinary Core Words in one flat dictionary, reached by their plain names, with contracts, laws, and conformance held to the same standard. Growth is not the goal: a proposed Word that is expressible as a user definition over the existing vocabulary does not belong in Core — unless expressing it that way costs asymptotically more than the same work done in the kernel, in which case what the definition demonstrates is a gap in the vocabulary rather than the absence of one.
</p>

<h3 id="lang-authority-freedom">LANG.AUTHORITY.FREEDOM — Implementation freedom</h3>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Category</th><th>Examples</th></tr></thead><tbody><tr><td>Unobservable — free to change when all observations and host protocol payload meanings stay unchanged</td><td>AST, IR, dispatch, caching, storage layout, numeric representation, optimization</td></tr><tr><td>Not a semantic discriminant</td><td>Internal exact-real representation, Rust enum names, debug strings, allocation identity, GUI colors, private serialization</td></tr></tbody></table></div>

<p>In particular an implementation may execute a Word by any route it likes, provided the route is unobservable.</p>

<h2 id="lang-source">2. Source and Desugaring</h2>

<h3 id="lang-source-text">LANG.SOURCE.TEXT — Source domain</h3>

<p>
A program is Unicode text tokenized by the sealed Ajisai lexical grammar (<code>spec/grammar.json</code>). There are five kinds of token: a number, a string, a name, and the two delimiters <code>[</code> and <code>]</code>. Everything else — which Word a name is, whether a Vector is code, what a definition is — is decided after tokenizing, not by it.
</p>

<p>
A decimal literal carries at least one digit on both sides of the point: <code>0.5</code> and <code>5.0</code> are numbers, <code>.5</code> and <code>5.</code> are not. The point is therefore never the first or last character of a number, so a lone <code>.</code> is unambiguously a name rather than a truncated literal.
</p>

<p>
This numeric grammar is the single definition of what text denotes a number. A conversion from Text to a Scalar accepts exactly the lexemes a source literal accepts and projects everything else to NIL, so the language has one numeric grammar rather than one per entry point. A pair whose denominator is zero, <code>1/0</code>, is a number like <code>1/2</code>: in source and as text alike it denotes one of the two points over zero (LANG.VALUES.EXACT): <code>1/0</code> for a nonzero numerator of either sign, <code>0/0</code> for zero. How large a number a lexeme may denote is bounded by the numeric-literal ceiling (LANG.MACHINE.LIMITS), counted in the digits it denotes rather than the characters it takes — <code>1e5000</code> denotes a 5001-digit integer — at every entry point that reads this grammar.
</p>

<p>
Whitespace is the <em>sole</em> token delimiter and is otherwise insignificant; a line break is whitespace like any other, so a program written across lines and the same program written flat are one program, with one Word identity (LANG.DICTIONARY.MUTATION). A line break ends a comment, which is the only thing it ends: <code>#</code> at the start of a word runs to the end of its line. No other character splits a token, the delimiters included: each of <code>[</code> and <code>]</code> must stand alone like every other word, so <code>[ 1 2 3 ]</code> is well-formed and <code>[1 2 3]</code> is a source error asking for the missing space, and <code>#</code> glued to a preceding lexeme is just part of that one name, not a comment. A quoted string is the one delimiter of its own: it may hold whitespace internally (<code>'hello world'</code>), but its closing <code>'</code> must itself be followed by whitespace or end of input to close.
</p>

<p>
An unclosed string, a delimiter glued to anything and an unbalanced delimiter are the source errors, and there are no others: every lexeme the other rules decline is a name, so no name is invalid as text. A source error refuses the whole program before any of it runs. It does not denote NIL.
</p>

<h3 id="lang-source-normalize">LANG.SOURCE.NORMALIZE — Name normalization</h3>

<p>
Word lookup is case-insensitive through the canonical normalization, and case is all it folds: a Word has exactly one name, and no second spelling resolves to it.
</p>

<p>
Normalization does not merge distinct value tags, invent dictionary entries, or change string and code-block contents.
</p>

<h3 id="lang-source-desugar">LANG.SOURCE.DESUGAR — Surface forms</h3>

<p>
Desugaring is deterministic and semantics-preserving. The registered delimiter forms lower to canonical concepts before evaluation.
</p>

<p>
If \(D\) is desugaring and \(O\) observation, then \(O(p)=O(D(p))\) for every well-formed program \(p\). Sugar cannot introduce a new value domain or failure category.
</p>

<h3 id="lang-source-code">LANG.SOURCE.CODE — Code values</h3>

<p>
Code is a Vector holding source for later evaluation — not a distinct domain from data, but the same Vector domain (LANG.VALUES.DISJOINT) read as executable by a Word whose contract requests it. <code>[ ]</code> is the sole bracket, for both: <code>[ 1 2 ADD ]</code> is equally a data literal and, wherever a Word's contract requests it, executable code. A value's construction history is not part of it (LANG.VALUES.DENOTATION), so nothing about how a Vector came to exist marks it as "code" or "data" ahead of use. Evaluated as code, a Vector's elements run in order: a Symbol names a Word, and every other element pushes itself exactly as it is held — a NIL with its reason, a Record, an exact irrational — whether or not any source text denotes it.
</p>

<p>
Producing, storing, displaying, and evaluating a Vector as code are distinct operations. Quoted code is not eagerly executed: evaluation occurs only through a Word whose contract requests it — <code>EXEC</code> and the higher-order family — never merely by building or holding the value; <code>CONTRACT</code> reads a block without evaluating it. Branching is not among them: <code>SELECT</code> chooses between two values the program has already built, so a branch evaluates nothing and skips nothing. A bare name written where a Vector element is being collected denotes a Symbol (LANG.VALUES.VECTOR) — data until something executes it — so a name is not resolved merely by appearing inside a literal. A Vector run as code is either <strong>written</strong> or <strong>built from data</strong>, and which one is decided where it runs, by its value and the code running it, never by how it was built. It is written when it equals a Vector literal of the code running it — the program, a Word's body, or the block whose code is running — or a Vector nested inside one of those literals, or when it names no Word anywhere inside it and so runs nothing. Anything else is built from data: a block taken apart and put back together, or one bound in another frame and run in this one. A written block is part of the code that runs it, so running it runs less code. A block built from data may run, but not while another block built from data is running: <code>EXEC</code>, <code>MAP</code>, <code>FILTER</code>, <code>FOLD</code> and <code>SCAN</code> refuse it as ERROR <code>nestedDataBlock</code>, before taking their operands. This is what keeps a block from running a copy of itself (LANG.DICTIONARY.ACYCLIC): <code>[ 1 COLLECT [ 0 0 ] GET EXEC EXEC ] 1 COLLECT [ 0 0 ] GET EXEC EXEC</code> is refused at its second run. The price is that code which runs a block it is given cannot be given a block that does the same: a Word that runs its operand runs <code>[ 2 ADD ]</code>, and refuses <code>[ [ 2 ADD ] W ]</code> when <code>W</code> is that Word. Code named by <code>DEF</code> does not pay it, since a Word call runs no block, so a computation needing more than one level of passed-in code is written as Words.
</p>

<h3 id="lang-source-frame">LANG.SOURCE.FRAME — What a block sees</h3>
<p>A block has no stack discipline of its own: the Word that evaluates it decides what the block reaches and what it may leave, and the block's text does not say which rule applies. The difference is whether an <code>ADD</code> written inside it finds two operands or none. Two rules cover every case.</p>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Rule</th><th>Applies to</th><th>Frame holds</th><th>On leaving</th></tr></thead><tbody><tr><td>Whole-stack</td><td>A user Word's body (<code>DEF</code>) and <code>EXEC</code></td><td>The whole stack</td><td>Leaves whatever it pushes, however many values</td></tr><tr><td>Isolated frame</td><td><code>MAP</code> <code>FILTER</code> — the current element · <code>FOLD</code> <code>SCAN</code> — the accumulator and the current element</td><td>A fixed number of values (one for <code>MAP</code> and <code>FILTER</code>, two for <code>FOLD</code> and <code>SCAN</code>)</td><td>Must leave exactly one; leaving none is ERROR, and anything below the top goes with the frame</td></tr></tbody></table></div>
<p>A block written inside another block is data where it is written, evaluated only when the Word receiving it runs it, under that Word's rule and not the enclosing block's. This holds regardless of what name the block writes: a name naming the word being defined, reached only through such a nested block, is still a reference to that word for LANG.DICTIONARY.ACYCLIC to see, even though nothing here evaluates it.</p>
<p>A frame also holds <strong>local bindings</strong>. <code>BIND</code> consumes a value and a name and makes the name that value for the rest of the frame, however many times it is written; several names destructure a Vector of the same length. The isolation above is of the stack, not of names, so the blocks a Word evaluates read the frame they were written in — and a Word call does not: a body reads its own bindings and its operands and nothing of its caller's, so what a Word means never depends on where it is called. A binding ends with its frame, and a name is a Word or a binding and never both.</p>

<h2 id="lang-values">3. Value Domains</h2>

<h3 id="lang-values-disjoint">LANG.VALUES.DISJOINT — Tagged domains</h3>

<p>
Values form a disjoint tagged sum of exactly seven domains: Scalar, Boolean, String, Vector, Record, NIL, and Symbol. A Record is a keyed correspondence (LANG.RECORDS.STRUCTURE); it is not a Vector, a Vector of pairs is not a Record, and nothing converts between them implicitly. A Symbol is a bare name, data until something executes it (LANG.SOURCE.CODE); it is its own domain, reachable standalone (<code>[ ADD ] 0 GET</code> leaves the Symbol <code>ADD</code> on the stack) as well as nested inside a Vector. Two values are never equal merely because their encodings resemble one another: a Symbol is not the String of the same spelling.
</p>

<p>
In particular FALSE is not scalar zero, TRUE is not scalar one, and an absent value is not an ERROR.
</p>

<h3 id="lang-values-denotation">LANG.VALUES.DENOTATION — Identity is denotation</h3>

<p>A value is what it denotes, never how it was made. Two values are one value exactly when they denote the same thing, whatever operations produced each of them: \(\sqrt{8}\) and \(\sqrt{2}+\sqrt{2}\) are one value, <code>100 0 DIV</code> and <code>5 0 DIV</code> are one value (<code>1/0</code>, LANG.VALUES.EXACT), and two NILs carrying one reason are one value. Construction history is not part of a value and cannot be read back out of one.</p>

<p>An internal representation is therefore unobservable, and a display is derived from the value rather than from the source that produced it. A procedure that decides identity answers about denotation, and where it cannot decide it answers that it cannot, never "different": comparison over the exact field decides (LANG.VALUES.EXACT), and the content identity of LANG.DICTIONARY.MUTATION decides only one direction. <code>spec/identity.json</code> states the law and what each level delivers.</p>

<h3 id="lang-values-exact">LANG.VALUES.EXACT — Exact scalars</h3>

<p>A scalar is an exact number. The domain is fixed by a condition rather than by a list: it is a field — closed under addition, subtraction, multiplication and division by anything but zero — whose equality and order an implementation decides in finite time through a normal form it exhibits, together with the two values that close it under division by zero, and the whole is the <strong>wheel of fractions</strong> over that field. A number is an integer pair taken up to a nonzero factor: \((a,b)\) and \((ka,kb)\) are one number for every \(k\neq 0\). Its normal form, the <strong>reduced pair</strong>, divides both halves by their greatest common divisor and makes the denominator non-negative; since the gcd of \(n\) and \(0\) is \(|n|\) and the factor \(-1\) relates \((1,0)\) and \((-1,0)\), every pair over zero reduces to one of exactly two — <code>1/0</code>, the one unsigned point at infinity, and <code>0/0</code>, the wheel's bottom. Nothing is added to the field by hand: the two are what the one normal form already holds, <code>100 0 DIV</code> and <code>-5 0 DIV</code> are both <code>1/0</code> for the reason <code>4/2</code> is <code>2/1</code>, and <code>1/0</code> is a literal as <code>1/2</code> is, with <code>-1/0</code> another spelling of it (LANG.SOURCE.TEXT).</p>

<p>The operations are the wheel's pair formulas — \(\frac{a}{b}+\frac{c}{d}=\frac{ad+bc}{bd}\), \(\frac{a}{b}\cdot\frac{c}{d}=\frac{ac}{bd}\), and the reciprocal \(/\frac{a}{b}=\frac{b}{a}\), with division as multiplication by the reciprocal — applied to every pair, a denominator of zero included, and reduced afterwards. Division is therefore <strong>total</strong>, every number has a reciprocal, and what follows is read off the formulas rather than decided separately: \(x\div 0\) is <code>1/0</code> for every nonzero \(x\) and <code>0/0</code> for zero; \(x+\frac{1}{0}\) is \(\frac{1}{0}\) for every finite \(x\), and \(\frac{1}{0}+\frac{1}{0}\) is \(\frac{0}{0}\); \(\frac{1}{0}\cdot x\) is \(\frac{1}{0}\) for every nonzero \(x\), and \(0\cdot\frac{1}{0}\) is \(\frac{0}{0}\); \(1\div\frac{1}{0}\) is \(0\); the negation of <code>1/0</code> is <code>1/0</code>; and \(\frac{0}{0}\) absorbs every operation. Over a pair with denominator zero only whether a finite operand is zero reaches the result, which is all an irrational operand contributes there: <code>2 SQRT 0 DIV</code> is <code>1/0</code>. Every axiom of a wheel holds at both points — the reciprocal is an involution, \(/(xy)=/x\,/y\), and the laws of addition carry the term \(0x\) that records where a point was met — while the laws of a field that mention zero, \(0\cdot x=0\), \(x-x=0\), \(x\div x=1\) and the distributive law, give way at the two points and nowhere else: <code>1 1 ADD 1/0 MUL</code> is <code>1/0</code> while <code>1/0 1/0 ADD</code> is <code>0/0</code>. An implementation must not restore them by special case, nor give <code>1/0</code> a sign; where they do hold is read off the contract instead (LANG.CONTRACT.FIELD), and their shapes are LANG.VALUES.SHAPE. <code>FLOOR</code> and <code>ROUND</code> leave the two points as they are, since no integer lies at them; <code>SQRT</code> answers each as its own root. Equality is decided everywhere — identity is denotation (LANG.VALUES.DENOTATION), so <code>0/0 0/0 EQ</code> and <code>1/0 -1/0 EQ</code> are TRUE — and order is decided over the field. Neither point has an order: <code>1/0</code> is unsigned and lies neither below nor above any number, and <code>0/0</code> lies nowhere, so a Word that asks for the order of either — <code>LT</code>, <code>GT</code>, <code>MIN</code>, <code>MAX</code>, <code>SORT</code>, <code>ORDER</code>, <code>BSEARCH</code> — has a well-formed operand outside the operation's domain and projects <code>domainMiss</code> (LANG.FAILURE.PROJECT). That is the one comparison the exact domain does not decide, and the second way UNKNOWN enters the truth domain (LANG.VALUES.TRUTH); every other comparison of two scalars yields TRUE or FALSE in finite time, not as a separate guarantee but as the half of the condition that fixes the domain.</p>

<p>The field \(\mathbb{Q}(\sqrt{d_1},\dots,\sqrt{d_k})\) generated over the rationals by square roots of non-negative rationals, in multiquadratic normal form \(\sum_d c_d\sqrt{d}\) with rational \(c_d\) and square-free integer \(d\), is the witness that meets the condition. Because every \(d\) is square-free the form is one per number, so a display and the protocol's <code>exactTerms</code> are read off the value and never off the operations that built it (LANG.VALUES.DENOTATION): <code>8 SQRT</code> and <code>2 SQRT 2 MUL</code> are one value written one way. Reaching the square-free part factors the radicand, which no cheap algorithm does for every integer, so that factoring is charged to the run's numeric work and a radicand the remaining work cannot factor is <code>resourceLimitExceeded</code> (LANG.MACHINE.LIMITS). Integer, fraction, decimal, and scientific-notation literals are source forms for exact rationals and the two points over zero, and <code>SQRT</code> generates the rest from a rational radicand: it is what builds the field rather than an operation the field is closed under. <code>POW</code> answers inside the domain — an integer exponent is repeated multiplication or division, so <code>0 -1 POW</code> is <code>1/0</code> as <code>1 0 DIV</code> is, and an exponent <code>p/2</code> over a non-negative rational base is a power of its square root; every other exponent, <code>1/0</code> included, leaves the field and projects <code>domainMiss</code>, while <code>0/0</code> absorbs <code>POW</code> in either operand as it absorbs every operation — and <code>GCD</code> and <code>RATIO</code> read the integers and the reduced numerator and denominator that every rational and every point over zero carries, <code>1/0 RATIO</code> being <code>[ 1 0 ]</code>; an irrational carries no such pair, so a total division widening the domain by two points makes no irrational a ratio, and <code>RATIO</code> projects <code>domainMiss</code> for it as <code>GCD</code> does for a non-integer. Arithmetic performs no intermediate rounding, and coefficients are arbitrary-precision, so a coefficient grows to whatever size the value requires. Rounding happens only where a program names it and only into text: <code>FORMAT</code> renders a scalar as decimal text with a stated number of digits, a tie rounding away from zero as <code>ROUND</code> rounds, and the field's decidable order settles every digit, an irrational's last one included; no decimal lies at the two points over zero, so <code>FORMAT</code> projects <code>domainMiss</code> for them, as <code>STR</code> does for an irrational. A real with no exhibited normal form is not an Ajisai value: widening the domain means exhibiting a new normal form that meets the condition, not amending the condition.</p>

<h3 id="lang-values-truth">LANG.VALUES.TRUTH — Three-valued truth</h3>

<p>The Boolean domain has exactly three truth values, those of the strong Kleene logic SQL uses: TRUE, FALSE and UNKNOWN. A truth value says what a program has been told: TRUE that something holds, FALSE that it does not, and UNKNOWN neither. TRUE and FALSE are the two Boolean data values; UNKNOWN is not a third variant but NIL (LANG.VALUES.NIL) read in truth position — a Word with nothing to decide from produces NIL, and NIL standing where a truth value is expected reads as UNKNOWN. <code>AND</code> and <code>NOT</code> compose all three by the tables below, and so does every connective written from them — a disjunction is <code>a NOT b NOT AND NOT</code>; composition is not passthrough, so an absent operand yields UNKNOWN only where the other operand does not already settle the result by itself.</p>
<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th><code>AND</code></th><th>TRUE</th><th>FALSE</th><th>UNKNOWN</th></tr></thead><tbody><tr><td>TRUE</td><td>TRUE</td><td>FALSE</td><td>UNKNOWN</td></tr><tr><td>FALSE</td><td>FALSE</td><td>FALSE</td><td>FALSE</td></tr><tr><td>UNKNOWN</td><td>UNKNOWN</td><td>FALSE</td><td>UNKNOWN</td></tr></tbody></table></div>
<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th><code>NOT</code></th><th>TRUE</th><th>FALSE</th><th>UNKNOWN</th></tr></thead><tbody><tr><td></td><td>FALSE</td><td>TRUE</td><td>UNKNOWN</td></tr></tbody></table></div>

<p><code>AND</code> is the meet of the truth order FALSE &lt; UNKNOWN &lt; TRUE: FALSE dominates even against an UNKNOWN operand, and TRUE leaves the other operand as it is. <code>NOT</code> exchanges TRUE and FALSE and leaves UNKNOWN where it is. Where the output is UNKNOWN it carries whichever operand's absence reason applies — the left operand's, when both are absent (LANG.FAILURE.PASSTHROUGH's left-to-right rule) — so the reason says why the truth is unknown: <code>domainMiss</code> for an order nothing decides, <code>conflict</code> for two sources that disagree, and so on.</p>
<p>UNKNOWN enters the truth domain three ways. An absent operand is read in truth position. An order is asked of a point over zero, <code>1/0</code> or <code>0/0</code>, which has none and projects the absence that then reads as UNKNOWN (LANG.VALUES.EXACT); the point itself is a number and never an UNKNOWN — <code>0/0 NIL?</code> is FALSE and <code>0/0 0/0 EQ</code> is TRUE — and only the question of its order is the absence <code>0/0 1 LT NIL?</code> finds. And two sources disagree: <code>RECONCILE</code> answers what two sources agree on — equal values agree, an absence yields to what the other source holds, and <code>0/0</code> yields to any other number as the bottom of the zero shapes (LANG.VALUES.SHAPE) — and any two different values, two truth values included, have nothing they agree on, so <code>RECONCILE</code> projects <code>conflict</code> (LANG.FAILURE.PROJECT): <code>TRUE FALSE RECONCILE</code> reads as UNKNOWN and its reason says the sources disagreed. That absence alone is not settled by a third source, so <code>RECONCILE</code> is associative, commutative and idempotent and a fold of it over many sources does not depend on their order. Every other comparison over the exact domain decides in finite time and yields TRUE or FALSE: that is a domain property of the field (LANG.VALUES.EXACT), not a limit this clause imposes. <code>SELECT</code> given UNKNOWN chooses neither candidate and answers the absence, and <code>FILTER</code> keeps only the elements its predicate calls TRUE, as an SQL WHERE clause keeps only the rows whose condition is true. Misuse still lives outside this domain: an operation that is malformed raises ERROR, which is never a truth value. The host protocol observes the two Boolean values as <code>boolean</code> nodes carrying <code>semantics.truthValue</code> <code>"true"</code> or <code>"false"</code>, and UNKNOWN as the NIL it is — a <code>nil</code> node carrying its absence reason (LANG.OBSERVATION.PROTOCOL); a consumer reads these rather than display text (LANG.OBSERVATION.FIREWALL). Being read in truth position adds an observation; it takes none away. UNKNOWN is still the NIL it is: <code>NIL?</code> answers TRUE for it, <code>NIL-REASON</code> reports the reason it arrived with, a fallback can be chosen in its place, and a passthrough Word passes it on — so <code>'x' NUM TRUE AND</code> is an UNKNOWN whose reason is still <code>invalidEncoding</code>. An implementation that reports UNKNOWN as present, or that drops its reason, contradicts LANG.VALUES.NIL.</p>

<h3 id="lang-values-shape">LANG.VALUES.SHAPE — Zero-shape arithmetic</h3>

<p>A number's <strong>zero shape</strong> is whether its numerator and its denominator are nonzero (LANG.VALUES.EXACT): <code>0</code>, the point <code>1/0</code>, <code>0/0</code>, and every other number, finite and nonzero. Ordered by how much each tells — a nonzero half tells more than a zero one — <code>0/0</code> is the bottom, finite nonzero the top, and <code>0</code> and <code>1/0</code> lie between, beside each other. The pair formulas read over those two-valued halves grade the arithmetic by the shapes: a product's shape is the meet of its operands' shapes, halfwise AND — <code>1/0 0 MUL</code> is <code>0/0</code>; the reciprocal exchanges the halves, swapping <code>0</code> and <code>1/0</code> and fixing the others; and a sum's shape is \((a\land d)\lor(b\land c)\) over \(b\land d\), exact everywhere but where two finite nonzero numbers cancel to <code>0</code>, so a point absorbs every finite number and <code>1/0 1/0 ADD</code> is <code>0/0</code>. Each shape is a group under <code>MUL</code> — the field's nonzero numbers, <code>1/0</code> alone, <code>0</code> alone and <code>0/0</code> alone — so the numbers are four groups laid over the shapes, and a division by zero steps from one layer to another rather than out of the domain. The distributive law fails only with <code>1/0</code> as the multiplier.</p>
<p>The zero shapes are an abstract domain, not a truth domain. They say which Words can leave the field (LANG.CONTRACT.FIELD): only a quotient, a power and a parse can answer a point over zero from finite operands. That <code>0/0</code> is the bottom of the shapes, as UNKNOWN is the bottom of what a program has been told, does not make it an UNKNOWN: it is a number (LANG.VALUES.TRUTH), and no Word converts a number into a truth value (LANG.VALUES.DISJOINT). The Words divide along the two orders: the arithmetic and <code>RECONCILE</code>, which act on what is known of a value, are the <code>knowledgeOrder</code> family and cite this clause; the truth values, their connectives and the Words that ask an order — <code>EQ</code>, <code>LT</code>, <code>GT</code>, <code>MIN</code>, <code>MAX</code> — are the <code>truthOrder</code> family and cite LANG.VALUES.TRUTH; an order asked of a point is where the two meet.</p><div class="informative"><p><strong>Informative.</strong> This block explains the clause and defines nothing (LANG.AUTHORITY.SOURCES).</p><p><em>Where the arithmetic sits.</em> The domain is Carlström's wheel of fractions over the integers, extended to the algebraic numbers <code>SQRT</code> builds. Anderson's transreal arithmetic totalizes division over the same pairs but keeps two signed infinities and sends \(\infty+\infty\) to \(\infty\); that keeps an order at infinity and gives up the reciprocal's involution, since both infinities have the one reciprocal <code>0</code>. Ajisai takes the wheel's side: with one zero, a signed infinity and an involutive reciprocal exclude each other, and the wheel keeps the involution and every wheel axiom at the price of an order at infinity.</p><p><em>The shapes and the truth values.</em> Read as two bits, the four shapes form the lattice Belnap called FOUR, and a product's shape is its knowledge meet. The truth domain is not that lattice: it is the three values of strong Kleene logic, which is FOUR without the value told both true and false (LANG.VALUES.TRUTH). The resemblance is why <code>0/0</code> and UNKNOWN both sit at the bottom, and it stops there.</p></div>

<h3 id="lang-values-nil">LANG.VALUES.NIL — Diagnostic absence</h3>

<p>
NIL is a value representing absence from a well-formed partial operation. It carries a <strong>reason</strong>: a stable, machine-readable identifier for why production failed. The reason is observable through <code>NIL-REASON</code> and through the protocol. The reason space has two layers: the closed set of identifiers <code>spec/outcomes.json</code> registers, and one of them, <code>userDeclared</code>, which a program reaches by <code>ABSENT</code> and which carries the Text the program gave as its parameter — that Text is what <code>NIL-REASON</code> answers for it.
</p>

<p>The reason is the entire observable content of a NIL, the declared Text of a <code>userDeclared</code> NIL included; so two <code>ABSENT</code> NILs are the same value exactly when their Texts are equal. An implementation may emit richer diagnostics on the host channel, and no program behavior may depend on them.</p>

<h3 id="lang-values-order">LANG.VALUES.ORDER — Order and grouping</h3>

<p>Two values have an order when both are numbers or both are Strings. Numbers are ordered by the exact field (LANG.VALUES.EXACT), where neither point over zero has a place. Strings are ordered by their Unicode scalar values, read left to right, a proper prefix before every longer String that extends it — SQL's binary collation, the same on every host because no locale, case folding or normalization enters it: <code>'abc' 'abd' LT</code>, <code>'Z' 'a' LT</code> and <code>'ab' 'abc' LT</code> are TRUE. Any other pair, a number against a String included, has no order, and a Word that asks for one — <code>LT</code>, <code>GT</code>, <code>MIN</code>, <code>MAX</code>, <code>SORT</code>, <code>ORDER</code>, <code>BSEARCH</code> — raises <code>nonNumeric</code>, wherever in its operands the pair stands: a point over zero met first does not turn that misuse into its <code>domainMiss</code> (LANG.FAILURE.TRICHOTOMY). A NIL is never asked its order. An absent operand of a comparison passes through (LANG.FAILURE.PASSTHROUGH), and <code>SORT</code> and <code>ORDER</code> place every NIL element after every present one, the NILs in their original order — SQL's ORDER BY … NULLS LAST, with <code>SORT REVERSE</code> the descending order that puts them first — so <code>[ 3 NIL 1 ] SORT</code> is <code>[ 1 3 NIL ]</code>: a sort drops nothing, and the permutation <code>ORDER</code> answers keeps every other column of a table aligned. <code>BSEARCH</code> reads an ascending Vector in this order, and no key finds a NIL in it, as SQL's = NULL matches no row. Grouping asks a different question from <code>EQ</code>. <code>UNIQUE</code>, <code>TALLY</code> and <code>GROUP</code> make two elements one key when they are one value (LANG.VALUES.DENOTATION), with one exception that SQL's DISTINCT and GROUP BY make too: every NIL, at any depth, is one key whatever its reason. A null a source stored and a key a record lacked therefore fall in one group, whose key is the first NIL met, its reason kept. A Record's keys are the same keys (LANG.RECORDS.STRUCTURE): two NIL keys repeat whatever their reasons, <code>MERGE</code> finds a NIL key under another reason, and the key keeps the spelling it was first given. <code>EQ</code> and <code>RECONCILE</code> still tell two NILs apart by their reasons (LANG.VALUES.NIL): two NILs held inside Vectors are equal exactly when their reasons agree.</p>

<h3 id="lang-values-vector">LANG.VALUES.VECTOR — Vectors</h3>

<p>
A Vector is an ordered finite collection of values. Indexing is 0-origin and negative indices count from the end. Vectors nest, and nesting expresses ragged and grouped data.
</p>

<p>
Vector length is semantic even when storage is flattened, shared, or lazily materialized. Order and length are a Vector's whole observable structure, and a nested Vector is an element like any other.
</p>

<p>
Inside a Vector literal a name denotes a Symbol (LANG.VALUES.DISJOINT): data until something executes it, and building the literal is not itself execution, so no dictionary lookup occurs there. <code>[ FOO ]</code> is the one-element Vector holding the Symbol <code>FOO</code> whether or not <code>FOO</code> is a defined Word, so a Vector literal denotes the same value under every dictionary state. Only <code>TRUE</code>, <code>FALSE</code> and <code>NIL</code> denote values rather than a Symbol carrying their name. A consequence worth stating: a misspelled name inside a Vector literal is a Symbol element, not an error.
</p>

<h3 id="lang-records-structure">LANG.RECORDS.STRUCTURE — Records</h3>

<p>A Record is a keyed correspondence: a sequence of distinct keys, each any value, paired position by position with a sequence of values. Those two sequences are its whole observable structure — <code>KEYS</code> and <code>VALUES</code> read them back, aligned — so two Records are one value exactly when their key sequences and their value sequences are (LANG.VALUES.DENOTATION), and key order is observable: <code>PUT</code> replaces a present key in place and appends an absent one, and <code>MERGE</code> keeps the left operand's order, takes the right operand's value where both hold a key, and appends the right's remaining keys. <code>RECORD</code> builds one from a Vector of keys and a Vector of values, and raises two ERRORs rather than making a choice for the program — a key left without a value under it is the length mismatch between the two sequences, and a repeated key is <code>duplicateKey</code>, keys being one key exactly when grouping makes them one (LANG.VALUES.ORDER). A Record is read and written by key with the same two Words that read and write a Vector by index, <code>GET</code> and <code>PUT</code>. Reading or removing an absent key (<code>GET</code>, <code>WITHOUT</code>) projects <code>notFound</code>; <code>HAS?</code> asks presence alone, so a stored NIL is told apart from an absent key. The key operand of these four Words is a <code>leaf</code>, as an index is, so a Vector of keys names several keys at once (LANG.COLLECTIONS.LIFT) and a key that is itself a Vector, a Record or NIL — which <code>RECORD</code>, <code>TALLY</code> and <code>GROUP</code> accept like any other — is not addressed by key: its value is read through the two sequences, <code>KEYS</code> and <code>VALUES</code>, comparing each key whole with <code>EQ</code>. A Record in a <code>leaf</code> or <code>truth</code> operand lifts the Word over its values, keys kept (LANG.COLLECTIONS.LIFT); in a <code>data</code> operand of a Word that reads a Vector it raises the operand ERROR the contract declares, and a Record Word given anything else raises <code>nonRecord</code>. <code>TALLY</code> and <code>GROUP</code> answer Records, keyed by the distinct elements and the keys they bundle by. <code>JSON-DECODE</code> reads JSON text into these domains — an object is a Record, an array a Vector, a number the exact rational it spells, <code>null</code> a NIL — projecting <code>invalidEncoding</code> for text that is not one JSON value and refusing nesting past the machine's bound as <code>resourceLimitExceeded</code> (LANG.MACHINE.LIMITS); <code>JSON-ENCODE</code> writes the inverse, spelling a rational with no finite decimal as its lexeme inside a string rather than rounding it, and projects <code>domainMiss</code> for a value with no JSON image.</p>

<h2 id="lang-machine">4. Machine State and Evaluation</h2>

<h3 id="lang-machine-state">LANG.MACHINE.STATE — State</h3>

<p>
A machine state contains the data stack, the dictionary, the output stream, and the execution controls needed by observable contracts.
</p>

<p>
Host-only caches, allocation arenas, compiled plans, and counters are not semantic state.
</p>

<h3 id="lang-machine-transformers">LANG.MACHINE.TRANSFORMERS — Programs</h3>

<p>Each executable token denotes a partial state transformer. A program denotes left-to-right composition of those transformers after desugaring and name resolution.</p>

<p>Execution is deterministic relative to the initial state. Optimization may reassociate internal work only when the observable sequence is unchanged.</p>

<h3 id="lang-machine-word-contract">LANG.MACHINE.WORD — Word contracts</h3>

<p>A canonical Word contract selects a semantic family and supplies its differences: stack arity, NIL policy, projection condition and reason, error conditions, purity, determinism, effects, clause links, documentation, and executor key. Determinism classifies what the result is relative to: <em>deterministic</em> from operands alone, <em>state-relative</em> when the wider stack, frame bindings, or dictionary also decide it (LANG.MACHINE.STATE) — a pure Word can still be state-relative, since purity (LANG.EFFECTS.OUTPUT) asks only whether the same stack and dictionary always yield the same result — or <em>host-relative</em> when the host's own rendering, capture, or discard of the effect also decides it (LANG.EFFECTS.OUTPUT).</p>

<p>The executor must refine its contract. Documentation is a projection of the same canonical entry, not an independent semantic authority.</p>

<h3 id="lang-machine-order">LANG.MACHINE.ORDER — Evaluation order</h3>

<p>Token evaluation, output emission, and dictionary mutation preserve their observable order.</p>

<h3 id="lang-machine-limits">LANG.MACHINE.LIMITS — Work limits</h3>

<p>A host bounds a run along several axes, and what distinguishes them is not which resource they meter but which of the three outcomes (LANG.FAILURE.TRICHOTOMY) exhausting one produces:</p>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Ceiling</th><th>Bounds</th><th>On exhaustion</th></tr></thead><tbody><tr><td>Execution-step limit</td><td>Total work across the run</td><td>ERROR, category <code>executionLimitExceeded</code></td></tr><tr><td>Materialization ceiling</td><td>Size of one generated collection</td><td>ERROR, category <code>resourceLimitExceeded</code>, naming <code>materializedElements</code></td></tr><tr><td>Call-depth ceiling</td><td>How many User Word calls and block evaluations (<code>EXEC</code>, <code>MAP</code>, <code>FILTER</code>, <code>FOLD</code>, <code>SCAN</code>) are active at once, counted together — a chain of distinct Words each calling the next, blocks nested inside them, or blocks nested inside them</td><td>ERROR, category <code>resourceLimitExceeded</code>, ceiling <code>callDepth</code></td></tr><tr><td>Value and work ceilings</td><td>One value or input (source bytes, the digits a numeric literal denotes, integer width, algebraic term count, nesting depth), or the numeric and collection work the run has accumulated</td><td>ERROR, category <code>resourceLimitExceeded</code></td></tr></tbody></table></div>

<p>Every row is the host refusing to continue, and the category each row names is normative: how many ceilings a host divides the last row into, and every numeric value, is implementation freedom. A ceiling belongs to the host, not to the program, so crossing one is never a value — not a NIL, which would flow on like any other value (LANG.FAILURE.PASSTHROUGH) and let one program answer <code>ok</code> with different values on two conforming hosts, each choosing a different branch of <code>NIL? SELECT</code>. An ERROR instead stops the run where the ceiling fired and names it (the ceiling, its value, and the size that crossed it), so a difference between hosts is always visible as a failure and never as a different answer, and a value a program observes denotes the same thing on every host (LANG.VALUES.DENOTATION). A Word asked to build a result past a ceiling — <code>RANGE</code>, <code>FILL</code>, <code>RESHAPE</code>, <code>JSON-DECODE</code>, <code>CHARS</code> and <code>TOKENIZE</code> past the materialization or nesting ceiling (LANG.COLLECTIONS.BUDGET), <code>NUM</code> or <code>JSON-DECODE</code> past the digits of a number, <code>POW</code> past the width of a number — refuses before building, by the same category and the same ceiling name as the same size reached any other way, and leaves its operands where they were. These are safety controls, not semantic constraints — termination already follows from LANG.DICTIONARY.ACYCLIC and LANG.SOURCE.CODE, so they bound cost rather than decide it.</p>

<h2 id="lang-stack">5. Stack and Consumption</h2>

<h3 id="lang-stack-order">LANG.STACK.ORDER — Stack observation</h3>

<p>The stack is an ordered sequence with a distinguished top. A Word takes its operands from the top and returns its results to the top.</p>

<p>A display transformation cannot reorder, coerce, drop, or invent stack values.</p>

<h3 id="lang-stack-consumption">LANG.STACK.CONSUMPTION — Consumption</h3>

<p>A Word consumes the operands it reads: they leave the stack, and its results take their place. Nothing modifies this, and a Word whose result is empty is no exception — <code>BIND</code>, <code>DEF</code> and <code>DEL</code> consume their operands too. A value used more than once is named with <code>BIND</code> and read by that name as often as it is needed: <code>5 'N' BIND N N 1 ADD</code> leaves <code>5 6</code>. Because every call consumes exactly what it reads, writing a User Word's body in place of the Word never changes which operands are consumed, which is what lets a program be expanded into Core Words alone.</p>

<p>A Word selects operands from the top of the stack, validates its registered contract, computes or projects the result, and then consumes its operands. ERROR does not masquerade as a successful NIL projection.</p>

<h2 id="lang-failure">6. Partiality and Failure</h2>

<h3 id="lang-failure-trichotomy">LANG.FAILURE.TRICHOTOMY — Value, absence, misuse</h3>

<p>Every attempted operation ends in exactly one of three categories:</p>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Category</th><th>Outcome</th></tr></thead><tbody><tr><td>Success</td><td>Its registered outputs</td></tr><tr><td>Well-formed partial failure</td><td>NIL with a reason</td></tr><tr><td>Malformed use</td><td>ERROR</td></tr></tbody></table></div>

<p>An implementation must not convert malformed use to NIL and must not raise ERROR merely because a registered partial projection has no value. Recovery operates on absence alone: a program can choose a fallback in place of a NIL, while an ERROR propagates and halts evaluation. A program declares either outcome itself: <code>ABSENT</code> makes a NIL whose reason is the Text it is given, and <code>FAIL</code> raises an ERROR (category <code>declaredFailure</code>) whose message is the Text it is given. A declared ERROR is an ERROR in full — no Word catches it — so the trichotomy is closed against the program as well as against the implementation.</p>

<h3 id="lang-failure-project">LANG.FAILURE.PROJECT — Projection</h3>

<p>A projection condition is a semantic predicate over well-formed inputs. When it holds, the Word produces NIL with the reason its contract registers.</p>

<p>Domain exclusion, out-of-range indexing, a value not found where one is sought, failed parsing, and space exhaustion are among the reasons a projection condition keeps distinct — <code>spec/outcomes.json</code> (LANG.AUTHORITY.SOURCES) is the exhaustive list, this clause only fixes how the shared ones behave. A Word's own contract names the reasons that Word projects for, which is a subset: a reason exists in the outcome space whether or not any contract currently reaches it.</p>

<h3 id="lang-failure-error">LANG.FAILURE.ERROR — Errors</h3>

<p>Arity failure, nonconforming type, malformed source, invalid dictionary operation, and an exhausted execution-step limit raise their registered ERROR category.</p>

<p>ERROR halts evaluation and propagates. It is never a truth value and never a stack value.</p>

<h3 id="lang-failure-passthrough">LANG.FAILURE.PASSTHROUGH — NIL passthrough</h3>

<p>What a Word does with a NIL operand follows from what it does with that operand, and from nothing else. Each operand position has one role, declared per Word as <code>stack.operands</code> in <code>spec/words.json</code>. A <strong>data</strong> or <strong>leaf</strong> operand is read (a leaf as one value that is not a container — a Scalar, String, Boolean or Symbol — lifted over a Vector or Record, LANG.COLLECTIONS.LIFT): an absent one is the result, flowing to the output position without changing its reason; the primitive does not run, no projection condition is re-run or relabels it, and when several data operands are absent the leftmost is the result. An <strong>element</strong> is carried without being read (a value stored, bound, printed, encoded or inspected, an accumulator, a needle compared by equality): a NIL there is an ordinary value (LANG.VALUES.NIL). A <strong>control</strong> operand directs the Word rather than being data it transforms — a block it evaluates, a name it binds, defines, deletes or looks up, or a message it raises: it cannot be absent, so a NIL there is malformed use, raising the condition the Word declares for any other operand that does not belong there, and this is decided before any data operand passes through. A <strong>truth</strong> operand reads a NIL as UNKNOWN (LANG.VALUES.TRUTH).</p>

<p>A Word's <code>nilPolicy</code> summarises its roles and is never a choice of its own, so two Words that treat an operand alike treat a NIL there alike. A Word of data-dependent arity declares no roles and states its NIL handling in its contract.</p>

<h3 id="lang-failure-recovery">LANG.FAILURE.RECOVERY — Recovery</h3>

<p>Recovery is a phrase, not a form of its own. <code>NIL?</code> consumes a value and answers whether it is absent, which is what <code>SELECT</code> reads as its truth operand, so a subject named once and read twice chooses between itself and a fallback: <code>subject 'S' BIND fallback S S NIL? SELECT</code> leaves the subject when it is present and the fallback when it is not, the fallback an ordinary operand computed before the choice like every other operand. The question is asked of the whole value: a Vector holding an absent lane is present, so a lane recovered inside a Vector is recovered there rather than around it.</p>

<p>Recovery does not erase absence from already emitted output.</p>

<h2 id="lang-collections">7. Collections and Higher-order Evaluation</h2>

<h3 id="lang-collections-lift">LANG.COLLECTIONS.LIFT — Element lifting</h3>

<p>A Word applies element-wise wherever it reads an operand as one value: a <code>leaf</code> operand, read as one value that is not a container — a Scalar, String, Boolean or Symbol — and a <code>truth</code> operand (LANG.FAILURE.PASSTHROUGH). Given a Vector there, it answers a Vector of its answers for the elements; given a Record, a Record of its answers under the unchanged keys (LANG.RECORDS.STRUCTURE). Every Word lifts by this one rule, arithmetic, comparison, logic and text alike, and a one-element Vector is a Vector there, never its element. A scalar combines with every element of a vector, however that vector is nested, including a ragged one. A Record lifts first: any other operand combines with each of its values, two Records combine value by value when their key sequences are equal, and two whose key sequences differ are a <code>shapeMismatch</code> ERROR.</p>

<p>Two vectors combine by pairing their axes. A vector whose nesting is rectangular has a shape: the lengths of its axes, outermost first. The two shapes are aligned at their innermost axis, and an axis the shorter shape does not reach counts as length 1. Paired axes combine when their lengths are equal, and when one of them is 1 that operand's single lane is reused across the other's length; the result carries the longer length on that axis. This makes a one-element vector combine with a vector of any length, and <code>[ 1 2 3 ] [ 10 ] MUL</code> is <code>[ 10/1 20/1 30/1 ]</code>. Any other pairing is ERROR, and so is any pairing of two vectors where either one is ragged. A program reads this shape with <code>SHAPE</code>, which answers a rectangular vector's axis lengths and, for a ragged one, the reasoned absence <code>domainMiss</code> — a ragged vector has no shape; <code>RESHAPE</code> regroups a vector's leaves, in order, under a shape whose product is their count; <code>FLATTEN</code> collapses every axis into one; <code>DEPTH</code> answers how deeply a value nests, a leaf being 0. The last two cannot be written as user definitions: nesting depth is not known in advance, and a language with no recursion and no unbounded loop cannot walk a structure of unknown depth (LANG.DICTIONARY.ACYCLIC).</p>

<p>Each lane preserves the exactness, truth, NIL, and ERROR distinctions of the scalar law. Vectorization cannot turn an ERROR lane into NIL.</p>

<h3 id="lang-collections-higher">LANG.COLLECTIONS.HIGHER — Higher-order evaluation</h3>

<p><code>MAP</code>, <code>FILTER</code>, <code>FOLD</code>, and <code>SCAN</code> evaluate their code operand (LANG.SOURCE.CODE) once per visited element, in index order, with the block's stack effect isolated to its own operands. A block reaches an inner axis by nesting: <code>[ [ f ] MAP ] MAP</code> applies <code>f</code> one level down. The code operand is written or built from data as LANG.SOURCE.CODE says, and one built from data is refused as <code>nestedDataBlock</code> while another runs.</p>

<p><code>FOLD</code> and <code>SCAN</code> are one walk over the elements, carrying an accumulator the block rewrites at each one; they differ in which accumulators the walk answers with. <code>FOLD</code> answers the last, so a walk with nothing to visit answers the seed it was given. <code>SCAN</code> answers every one of them, one per visited element and the seed not among them, so a walk with nothing to visit answers no elements and an absent collection answers that same absence. This is the one shape a computation carrying state from one element to the next can take, because a Word cannot call itself (LANG.DICTIONARY.ACYCLIC) and there is no unbounded loop.</p>

<p>Element visitation order is observable where a supplied block can emit output or mutate dictionary state. The block's result is what it leaves on top when it finishes, taken as it stands, whatever domain it is in: a Vector of one element is a Vector of one element (LANG.VALUES.DISJOINT), so <code>[ 1 2 ] [ 1 COLLECT ] MAP</code> answers <code>[ [ 1 ] [ 2 ] ]</code> and no Word unwraps a result on the grounds of its length. A block finishes having left <strong>exactly one</strong> value, and that value is its result: a block that leaves nothing, or more than one, raises the <code>blockContractViolation</code> its caller's contract registers, and the Word answers nothing. The rule is the same for every count other than one, because a discarded surplus is a quiet wrong answer of the kind LANG.FAILURE.TRICHOTOMY rules out: <code>[ 1 2 3 ] [ 1 GT TRUE ] FILTER</code> is an ERROR, not a predicate that keeps every element on the strength of a <code>TRUE</code> written above the comparison it was meant to make. A value computed along the way is consumed before the block finishes, or named with <code>BIND</code>, which leaves nothing on the stack.</p>

<h3 id="lang-collections-budget">LANG.COLLECTIONS.BUDGET — Materialization</h3>

<p><code>RANGE</code>, <code>FILL</code>, <code>RESHAPE</code> and <code>JSON-DECODE</code> honor the materialization ceiling, and so do <code>CHARS</code> and <code>TOKENIZE</code>, whose parts are a generated collection too; <code>FILL</code>, <code>RESHAPE</code> and <code>JSON-DECODE</code> honor the nesting ceiling as well, since the number of axes a shape names and the depth a JSON text nests are sizes of what they were asked to build. A well-formed request that cannot materialize within them is refused before anything is built, as ERROR with category <code>resourceLimitExceeded</code> naming the ceiling (<code>materializedElements</code> or <code>nestingDepth</code>), its value and the size requested (LANG.MACHINE.LIMITS); malformed dimensions remain ERROR under their own names. The refusal is never a value: a ceiling is the host's, and a program must not be able to observe one as an answer.</p>

<h2 id="lang-dictionary">8. Dictionary and Effects</h2>

<h3 id="lang-dictionary-resolution">LANG.DICTIONARY.RESOLUTION — Deterministic lookup</h3>

<p>The dictionary has two tiers, and those two are the whole of it. <strong>Core</strong> holds the 77 canonical Words and is sealed: a Core name cannot be redefined or deleted. <strong>User</strong> holds definitions made by <code>DEF</code>. Resolution is a deterministic function of the normalized name and the current dictionary, and User never shadows Core. The host's lookup, hover, the Reference, and execution must identify the same canonical entry.</p>

<h3 id="lang-dictionary-mutation">LANG.DICTIONARY.MUTATION — User Words</h3>

<p><code>DEF</code> binds a name to a code block; <code>DEL</code> removes a User Word and fails with ERROR if any other definition still depends on it, and <code>DEF</code> of a name that other definitions still depend on fails the same way (<code>definitionConflict</code>): a caller resolves the name when it runs, so the definition it calls cannot be changed under it, only deleted after the caller is. A definition depends on every User Word its body names, whichever of the two was defined first. A dictionary mutation commits atomically or raises ERROR with no partial visible mutation. A definition is kept as its source: <code>DEF</code> takes any Vector as a body, and a value that body carries whole rather than as written — a Record, an exact irrational, a Vector holding either — is written back as the source that builds it (<code>[ keys ] [ values ] RECORD</code>, the normal form over <code>SQRT</code>, <code>… n COLLECT</code>), so the body the dictionary holds, shows, saves and identifies is one text in every session; a NIL carrying a reason has no source, and a body holding one is refused (<code>invalidDefinitionBody</code>).</p>

<p>Every Word has a <strong>content identity</strong>: a digest over its normalized definition and the identities of the Words it calls, so a change to a dependency changes the identity of everything that depends on it, and what a Word or its dependencies are named does not reach it. Equal identities mean one Word, which is what lets a host deduplicate by content; unequal identities mean nothing, since two definitions can denote one function and normalize differently (LANG.VALUES.DENOTATION). <code>DIGEST</code> answers that identity for a Symbol naming a Word, and for any other value the digest of its denotation, under the same asymmetry. A String is such a value even when its text spells a Word's name, so a value's digest never depends on the dictionary.</p>

<h3 id="lang-dictionary-acyclic">LANG.DICTIONARY.ACYCLIC — No definition may name itself</h3>

<p>The User dictionary's reference graph is acyclic: <code>DEF</code> raises ERROR rather than commit a definition that names the word being defined, directly or through a chain of other User words, however that name is reached (LANG.SOURCE.FRAME), and <code>BIND</code> refuses a value that names its own binding, directly or through other bindings the frame can see, since a binding is looked up when its Vector runs and would otherwise call itself with no Word in between. Both checks read every Symbol the body or value holds, including one inside a Record or other value a body built from a Vector carries whole, and they are complete because a Word runs only when a Symbol names it: no Word turns text into a Symbol, and a String is not code (LANG.SOURCE.CODE), so a call can never be computed. No Word can call itself, and no block can run itself: a block runs the blocks written inside it, Words, and a block built from data only when no other is running (LANG.SOURCE.CODE). So repetition is only the bounded higher-order Words (<code>MAP</code>, <code>FILTER</code>, <code>FOLD</code>, <code>SCAN</code>) over an already-finite Vector, and every evaluation is structurally finite: termination follows from the dictionary's shape and the nesting of the code, and the execution-step ceiling (LANG.MACHINE.LIMITS) is left to bound cost, not decide it. Ajisai is therefore total and not Turing-complete — a deliberate trade, whose price is that a computation which must run until it converges has to be re-expressed over a finite Vector the program builds first, and whose return is that the language denotes without domain theory: no divergence, so no bottom, no continuity obligation and no fixed-point construction. <code>spec/termination.json</code> carries the argument in full.</p>

<h3 id="lang-effects-output">LANG.EFFECTS.OUTPUT — Output</h3>

<p>Output is the only effect that leaves the machine. <code>PRINT</code> consumes its operand and appends it to the ordered output stream. No other Word emits output, so the output stream is the whole of what a host observes a program doing.</p>

<p>A host may render, capture, or discard the output stream, but may not reorder it or change language-side validation.</p>

<p>A Word may have exactly one other effect, and it stays inside the machine: <code>DEF</code> and <code>DEL</code> change the dictionary, under LANG.DICTIONARY.MUTATION. Output emission and dictionary mutation are therefore the two effects, and LANG.MACHINE.ORDER orders both of them against token evaluation.</p>

<p>Every other Word changes nothing: given the same stack and dictionary it produces the same result. A Word that evaluates a supplied code block has the effects of that block and no others, so it is pure exactly when the block is.</p>

<h2 id="lang-contract">9. Contracts and Static Checking</h2>

<h3 id="lang-contract-registry">LANG.CONTRACT.REGISTRY — Machine-readable contracts</h3>

<p>Every Core Word's contract is a machine-readable record in <code>spec/words.json</code>, conforming to <code>spec/words.schema.json</code>. The record is the single place a Word's arity, NIL policy, projection reason, error conditions, purity, and documentation are stated.</p>

<p>Prose that restates a contract is a projection of that record and carries no independent authority. <code>CONTRACT</code> answers a contract from inside the language as a Record keyed <code>inputs</code>, <code>outputs</code> and <code>field</code>: the registered one for a Core Word, the inferred one of LANG.CONTRACT.CHECK for a User Word or a block of code, in the same keys and the same vocabulary. It names a Word by a String, as <code>DEF</code>, <code>BIND</code> and <code>DEL</code> do, or by a Symbol; reading a contract calls nothing, so a String there never becomes a call (LANG.SOURCE.CODE). For a name of no Word it projects <code>notFound</code>.</p>

<h3 id="lang-contract-check">LANG.CONTRACT.CHECK — Pre-execution check</h3>

<p>A user definition may carry a declaration of its own contract, written as keys of a contract Record with the values those keys take — <code>inputs</code>, <code>outputs</code> and <code>field</code>. The declaration is a comment line to the interpreter and a directive to the check: <code>#:contract DOUBLE inputs=1 outputs=1 field=closed</code>, one line per Word, naming the Word it declares. <code>inputs</code> and <code>outputs</code> must equal the inferred counts; <code>field</code> is a bound the inferred contract must not exceed; a key left out is not checked, and any other key makes the directive malformed. <code>ajisai check</code> verifies the declaration against the contracts of the Words the body calls, <strong>without running the program</strong>, and so does every host operation that would run it: a program whose declaration is violated is not executed, and is answered as an ERROR (<code>contractViolation</code>) with the findings beside it. The check reads one contract no program declares, the higher-order Word's own of LANG.SOURCE.FRAME: a block written immediately before <code>MAP</code>, <code>FILTER</code>, <code>FOLD</code> or <code>SCAN</code> whose inferred arity cannot leave exactly one value from those its Word gives it is answered as the ERROR running it raises, while a host that runs the program leaves it to the run, since over an empty collection the block never runs. <code>CONTRACT</code> reaches the same inference from inside the language, over a Vector of code (LANG.SOURCE.CODE) or the name of a User Word: it never evaluates its operand, so calling it carries none of the operand's own effects. Where the inference cannot decide, its answer is the weaker claim — a <code>'variable'</code> arity, or <code>'leaving'</code> — which is how the outcome <em>cannot verify</em> below reaches a program as data.</p>

<p>The check is deliberately <strong>conservative and partial</strong>. It reports exactly three outcomes per declaration, each the trichotomy of LANG.FAILURE.TRICHOTOMY applied at check time rather than at run time:</p>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Check-time outcome</th><th>Run-time counterpart</th></tr></thead><tbody><tr><td><em>Verified</em></td><td>A value — the inferred contract itself</td></tr><tr><td><em>Cannot verify</em></td><td>A reasoned absence</td></tr><tr><td><em>Violated</em></td><td>An error</td></tr></tbody></table></div>

<p>Anything outside the syntactic fragment the inference analyzes — a higher-order body whose block is not statically known, or dynamic control — is reported as <em>cannot verify</em> and is never silently passed. A tool that reports <em>verified</em> for an unanalyzable body is nonconforming.</p>
<p>The correspondence classifies outcomes, not mechanisms, and does not make the check evaluate the program: a negative radicand, a failed parse and an out-of-range index already share one outcome category while sharing no mechanism, and an inference that could not decide joins that list on the same terms.</p>

<h3 id="lang-contract-field">LANG.CONTRACT.FIELD — Where the field ends</h3>

<p>Division is total (LANG.VALUES.EXACT), so no run is ever stopped by a zero divisor, and the price of that is paid in one place: the two points over zero, where the field laws that mention zero — \(0\cdot x=0\), \(x-x=0\), \(x\div x=1\) and the distributive law — give way. What a program may assume therefore depends on one question, and the contract answers it before anything runs: <em>can this code answer a point over zero from operands that hold none?</em> Every contract carries the answer under the key <code>field</code>, with one of two values. A Word is <code>closed</code> when every number it answers lies in the field whenever every number it reads does; it is <code>leaving</code> when it can answer <code>1/0</code> or <code>0/0</code> from operands holding none of them. Among the Core Words exactly three are <code>leaving</code>, each because it reaches past the field by design rather than by failure: <code>DIV</code> (<code>1 0 DIV</code>), <code>POW</code> (<code>0 -1 POW</code>, the reciprocal of zero) and <code>NUM</code> (<code>'1/0' NUM</code>, a point spelled in text). Every other Word is <code>closed</code>: one that adds, multiplies, orders, rounds or takes roots keeps finite operands finite, and one that only carries numbers creates none. A Word that runs a block is <code>closed</code> itself; the Words of its block decide for it. For a User Word or a block the value is read off the body without evaluating any of it: the body is <code>leaving</code> as soon as one Word it calls is <code>leaving</code> or one literal it writes lies over zero, and <code>closed</code> otherwise. One operand is read where it is written: a number literal written immediately before <code>DIV</code> or <code>POW</code> is the very operand that decides it, so <code>[ 2 DIV ]</code> and <code>[ 2 POW ]</code> are <code>closed</code> while <code>[ 0 DIV ]</code>, <code>[ -1 POW ]</code> and <code>[ LENGTH DIV ]</code> — a length may be zero — stay <code>leaving</code>; an operand the body computes is not read, so <code>[ 2 SQRT DIV ]</code> is <code>leaving</code> too, the weaker claim. A literal over zero leaves as surely, <code>[ 1/0 MIN ]</code> as <code>[ 0 DIV ]</code>. Code a higher-order Word runs is read when it is the literal written immediately before that Word; code that reaches it any other way — a binding, a Vector taken out of data, an operand of the Word being defined — was never read, so the body counts as <code>leaving</code>, with no exit to name. A report of a <code>leaving</code> body names its exits, the calls that can leave the field and the literals written over zero, in the order the body writes them, and a violated <code>field=closed</code> names them too, so the finding says what to change. The answer is what makes the field laws usable again without taking total division back. Inside code that is <code>closed</code> and fed finite operands, every number is a number of the field and every law of the field holds, so a reader, a tool or an agent may rewrite \(a(b+c)\) as \(ab+ac\) or \(x-x\) as \(0\) there and change nothing; at a <code>leaving</code> boundary that licence stops, and only the laws that hold everywhere — addition and multiplication remain associative and commutative, and the reciprocal an involution, over both points — survive it. The same boundary is where the one bridge into the truth domain begins: a point over zero reaches an order Word only by way of a <code>leaving</code> Word or literal, so over finite operands a <code>closed</code> body never asks the order of a point, the one route by which arithmetic itself enters UNKNOWN (LANG.VALUES.TRUTH); an absent operand still reads as UNKNOWN wherever it stands. A declaration <code>field=closed</code> is a bound in the sense of LANG.CONTRACT.CHECK: an inferred <code>leaving</code> that names an exit violates it, one resting only on code the walk could not read cannot verify it, and <code>field=leaving</code> admits either.</p>

<h2 id="lang-observation">10. Observation and Host Protocol</h2>

<h3 id="lang-observation-projections">LANG.OBSERVATION.PROJECTIONS — Observable surfaces</h3>

<p>Ajisai exposes four projections: Input, Output, Stack, and Dictionary. Presentation may tile or select them according to the Presentation Profile, but their language-side contents are determined here.</p>

<div class="ref-table-wrap"><table class="ref-table"><thead><tr><th>Projection</th><th>Content</th></tr></thead><tbody><tr><td>Output</td><td>The ordered text observation</td></tr><tr><td>Stack</td><td>The ordered typed values</td></tr><tr><td>Dictionary</td><td>The resolved Core and User catalog</td></tr><tr><td>Input</td><td>Normalized source and host editing state, where applicable</td></tr></tbody></table></div>

<h3 id="lang-observation-protocol">LANG.OBSERVATION.PROTOCOL — The host protocol</h3>

<p>One recursive node shape observes every stack value, rendered by a single serializer shared across hosts (<code>spec/host-protocol.schema.json</code>): a <code>type</code> naming the value's domain (LANG.VALUES.DISJOINT's seven domains as seven wire types — <code>nil</code>, <code>boolean</code>, <code>number</code>, <code>string</code>, <code>vector</code>, <code>symbol</code>, and <code>record</code>, the last carrying its aligned key and value sequences as two arrays of nodes), a <code>value</code> shaped by that type, and an optional <code>semantics</code> bag carrying a Boolean's truth, a NIL's absence reason, and an irrational's exact terms. Every field is derived from the value itself, never from the Word that produced it (LANG.VALUES.DENOTATION). The CLI/MCP agent surface wraps an array of these nodes in its own JSON envelope (<code>docs/dev/agent-cli-output-contract.md</code>, non-canonical); the WASM/GUI boundary exposes the same nodes directly from its own methods, with no enclosing envelope of its own. It is the only structured channel through which anything outside the language observes the stack; a text rendering such as the CLI's <code>stackDisplay</code> is a view of these nodes, not a second protocol. The schema is enforced: the MCP self-test validates every stack the live server returns against it.</p>

<p>No version field exists on this node shape today: the CLI envelope around it carries its own plain integer <code>schemaVersion</code>, and the WASM boundary carries no version signal at all, so a breaking change to the shape this clause fixes would be silent until one is added. Existing field deletion, rename, semantic change, and tuple reorder or reshape remain forbidden regardless: an implementation offers exactly one protocol.</p>

<h3 id="lang-observation-firewall">LANG.OBSERVATION.FIREWALL — Semantic firewall</h3>

<p>External consumers branch only on published protocol axes. They do not branch on Rust types, debug strings, display text, internal numeric form, or incidental GUI state.</p>

<p>The GUI never infers exact equality, stack effect, resolution, or absence reasons on its own.</p>

<h3 id="lang-observation-diagnosis">LANG.OBSERVATION.DIAGNOSIS — Diagnosis</h3>

<p>An ERROR carries a stable category identifier and a human-readable message; a NIL carries its reason. Those identifiers are the machine-readable surface, and human wording may evolve only where they are preserved.</p>

<h2 id="lang-conformance">11. Conformance</h2>

<h3 id="lang-conformance-corpus">LANG.CONFORMANCE.CORPUS — Executable correspondence</h3>

<p>The conformance corpus links each case to one or more clause IDs and compares stack, output, dictionary state, and diagnosis identifiers. An implementation is Ajisai when it preserves the source-to-observation correspondence for every case.</p>

<p>The corpus is the decision procedure for the question "is this Ajisai?". No prose answers it.</p>

<h3 id="lang-conformance-families">LANG.CONFORMANCE.FAMILIES — Family laws</h3>

<p>Every semantic family has law tests for arity, consumption, NIL policy, projection, ERROR boundaries, lifting, purity, and effects as applicable. Every Word contract has at least one conformance path to its family and clause IDs.</p>

<h3 id="lang-conformance-change">LANG.CONFORMANCE.CHANGE — Change discipline</h3>

<p>A semantic change begins in one authoritative source, regenerates all derived surfaces, updates clause-linked conformance cases, and demonstrates unchanged observations unless the change is explicitly versioned.</p>

<!-- INCLUDE:presentation-profile -->
