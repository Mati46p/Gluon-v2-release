# G-Protocol: Markdown Nesting & Code Block Formatting Fix

## Problem Description

Models (especially LLMs like Claude, GPT-4) have recurring issues with properly formatting code blocks in G-Protocol SEARCH/REPLACE operations. This leads to visual corruption and parsing failures.

## Technical Root Causes

### 1. **Backtick Conflicts (Markdown Nesting)**
- **Issue**: JavaScript/TypeScript template literals use backticks: `` `text` ``
- **Conflict**: Code fence markers also use 3 backticks: ` ``` `
- **Result**: Parser may interpret template literal backticks as fence closures, breaking the structure

**Example of failure:**
```javascript
console.log(`Error: ${message}`);
```  ← Parser may think fence closes here
╔═══════ SEARCH  ← This renders as plain text instead of staying in code block
```

### 2. **Missing Blank Line Separation**
- **Issue**: File path comment must be separated from Unicode box delimiters by a blank line
- **Without blank line**: Parser treats entire structure as comment text
- **Result**: Box delimiters (╔═══════) are not recognized, visual structure collapses

**Example of failure:**
```javascript
// File: src/app.js
╔═══════ SEARCH  ← No blank line = entire block treated as comment
```

### 3. **Unicode Box Character Rendering**
- **Issue**: Characters `╔═╗╚═╝` can be misinterpreted as table borders
- **When**: If they appear outside a code fence block
- **Result**: Markdown renderer tries to create tables, breaking visual structure

### 4. **Premature Fence Closure**
- **Issue**: Models sometimes insert closing fence ` ``` ` before completing the full SEARCH/REPLACE block
- **Result**: Remaining protocol instructions render as plain text
- **Causes**:
  - Confusion between template literals and fence markers
  - Incorrect line counting
  - Model "forgetting" it's inside a code block

## Solution: Strict Formatting Protocol

### ✅ Required Structure

```language
# File: path/to/file.ext

╔═══════ SEARCH
[search content]
╠═══════ REPLACE
[replace content]
╚═══════ END
```

### Critical Line-by-Line Breakdown

| Line # | Content | Critical Requirements |
|--------|---------|----------------------|
| 1 | ` ```language ` | Opening fence with language identifier (javascript, typescript, python, etc.) |
| 2 | File path comment | Use `//`, `#`, or `/* */` based on language |
| 3 | **BLANK LINE** | **ABSOLUTELY REQUIRED - NO EXCEPTIONS** |
| 4 | `╔═══════ SEARCH` | Unicode box delimiter - must be intact |
| 5+ | Search content | Code to find (with 3+ unique anchor lines) |
| N | `╠═══════ REPLACE` | Middle delimiter - separates search from replace |
| N+1+ | Replace content | Replacement code (preserve context lines) |
| M | `╚═══════ END` | Bottom delimiter - marks end of replace |
| M+1 | ` ``` ` | Closing fence - EXACTLY 3 backticks |

### Why Each Requirement Matters

#### Opening Fence with Language Identifier
```language ← MUST specify language
```
- **Purpose**: Tells parser this is code, not text
- **Effect**: Enables syntax highlighting and prevents markdown interpretation
- **Missing**: Everything inside renders as plain text

#### File Path Comment
```javascript
// File: src/app.js
```
- **Purpose**: Identifies target file for the apply system
- **Format**: Must use language-appropriate comment syntax
- **Position**: FIRST line inside fence (not outside!)

#### Blank Line (CRITICAL!)
```javascript
// File: src/app.js
                      ← THIS LINE IS CRITICAL
╔═══════ SEARCH
```
- **Purpose**: Separates metadata (file path) from protocol structure
- **Effect**: Ensures box delimiters are recognized as structural elements
- **Missing**: Parser treats delimiters as comment continuation, breaking structure

#### Unicode Box Delimiters
```
╔═══════ SEARCH   ← Top delimiter
╠═══════ REPLACE  ← Middle delimiter
╚═══════ END      ← Bottom delimiter
```
- **Purpose**: Visual and structural markers for SEARCH/REPLACE blocks
- **Why Unicode**: Avoids conflicts with Git-style `<<<<<<<` markers
- **Rendering**: Only safe when inside code fence

#### Closing Fence
` ``` `
- **Position**: MUST be last line (after `╚═══════ END`)
- **Count**: Exactly 3 backticks (no more, no less)
- **Timing**: Don't close early - ensure all content is inside fence first

## Examples

### ✅ CORRECT - JavaScript with Template Literals

```javascript
// File: src/logger.js

╔═══════ SEARCH
function logError(error) {
    console.log("Error occurred");
    return false;
}
╠═══════ REPLACE
function logError(error) {
    console.log(`Error: ${error.message}`);  // Template literal is safe here
    console.log(`Stack: ${error.stack}`);
    return false;
}
╚═══════ END
```

**Why this works:**
- ✅ Language identifier present (`javascript`)
- ✅ File path comment on line 2
- ✅ Blank line on line 3
- ✅ Template literals are INSIDE the fence (safe)
- ✅ All delimiters intact
- ✅ Fence closes AFTER ╚═══════ END

### ❌ INCORRECT - Missing Blank Line

```javascript
// File: src/logger.js
╔═══════ SEARCH
function logError(error) {
    return false;
}
╠═══════ REPLACE
function logError(error) {
    console.log(`Error: ${error.message}`);
    return false;
}
╚═══════ END
```

**Why this fails:**
- ❌ No blank line between file comment and `╔═══════ SEARCH`
- ❌ Parser treats delimiters as part of the comment
- ❌ Visual structure collapses
- ❌ Apply system cannot parse the block

### ❌ INCORRECT - Premature Fence Closure

```javascript
console.log(`test`);
```  ← Fence closed too early!
╔═══════ SEARCH  ← This renders as plain text
function logError(error) {
    return false;
}
```

**Why this fails:**
- ❌ Fence closed before SEARCH/REPLACE block started
- ❌ Box delimiters render as plain text or table elements
- ❌ Entire protocol structure is broken
- ❌ Apply system sees no valid blocks to process

## Verification Checklist

Before submitting any G-Protocol SEARCH/REPLACE block, verify:

### Format Integrity
- [ ] Opening fence has language identifier (` ```javascript `, ` ```typescript `, etc.)
- [ ] File path comment is FIRST line inside fence
- [ ] **BLANK LINE after file path comment (critical!)**
- [ ] Unicode box delimiters (╔ ╠ ╚) are present and intact
- [ ] All template literals and backticks are INSIDE the code fence
- [ ] Closing fence ` ``` ` is the LAST line (after ╚═══════ END)
- [ ] No stray backticks that could close fence prematurely

### Content Quality
- [ ] 3+ unique anchor lines before the change
- [ ] 3+ unique anchor lines after the change
- [ ] No generic anchors (`}`, `;`, `return`)
- [ ] File path is correct and absolute
- [ ] SEARCH block matches EXACT code (including whitespace/indentation)
- [ ] REPLACE block preserves context lines

## Implementation in Gluon

This fix has been integrated into:

1. **Frontend Protocol Instructions** ([prompt-generator.js](../sidebar/utils/prompt-generator.js))
   - Added as Section 0 (before all other rules)
   - Includes detailed technical explanation
   - Available in English and Polish

2. **Backend Protocol Definition** ([prompts.rs](../../../gluon-desktop/src-tauri/src/apply_system/prompts.rs))
   - Added as Section 0 in G_PROTOCOL_V2_SEARCH_REPLACE constant
   - Mirrors frontend instructions for consistency

3. **Verification Checklists**
   - Both frontend and backend checklists updated
   - Split into "Format Integrity" and "Content Quality" sections
   - Emphasizes critical formatting requirements

## Impact

With these changes:
- ✅ Models will generate properly formatted blocks
- ✅ Parsers can reliably extract SEARCH/REPLACE operations
- ✅ Visual rendering remains clean and readable
- ✅ Apply system success rate increases significantly
- ✅ Manual intervention for format fixes is minimized

## Model Training Notes

When training or prompting models to use G-Protocol:

1. **Emphasize the blank line requirement** - this is the most common mistake
2. **Show correct examples with template literals** - proves backticks can coexist safely
3. **Demonstrate premature closure failures** - helps models avoid early fence closing
4. **Use verification checklist** - encourages self-checking before submission
5. **Test with complex code** - ensure protocol works with nested structures and string interpolation

## References

- Original issue: Models frequently broke G-Protocol formatting with JavaScript template literals
- Root cause: Markdown nesting conflicts (backticks, missing separators, delimiter rendering)
- Solution: Strict line-by-line formatting protocol with mandatory blank line separator
- Version: G-Protocol v2 (SEARCH/REPLACE blocks with Unicode box delimiters)
