" Vim syntax file — patchlang (.phog)

if exists('b:current_syntax')
  finish
endif

" ── comments ──────────────────────────────────────────────────────────────
syntax match patchlangComment '//.*$'
highlight default link patchlangComment Comment

" ── top-level keywords ────────────────────────────────────────────────────
syntax keyword patchlangKeyword patch effect phrase voices poly mono legato
highlight default link patchlangKeyword Keyword

" ── node constructors ─────────────────────────────────────────────────────
syntax keyword patchlangNode osc adsr lpf hpf bpf notch delay mix amp lfo
highlight default link patchlangNode Function

" ── enum / value literals ─────────────────────────────────────────────────
" waveform shapes
syntax keyword patchlangValue sine square saw triangle
" duration names
syntax keyword patchlangValue eighth quarter half whole
highlight default link patchlangValue String

" ── implicit patch parameters ─────────────────────────────────────────────
syntax keyword patchlangParam freq velocity
highlight default link patchlangParam Special

" ── numbers ───────────────────────────────────────────────────────────────
syntax match patchlangNumber '\<\d\+\(\.\d\+\)\?\>'
highlight default link patchlangNumber Number

" ── pipe operator ─────────────────────────────────────────────────────────
syntax match patchlangPipe '\s*|\s*'
highlight default link patchlangPipe Operator

" ── arithmetic operators ──────────────────────────────────────────────────
syntax match patchlangOp '[+\-*/]'
highlight default link patchlangOp Operator

" ── brackets and delimiters ───────────────────────────────────────────────
syntax match patchlangDelim '[{}\[\]]'
highlight default link patchlangDelim Delimiter

syntax match patchlangComma ','
highlight default link patchlangComma Delimiter

" ── note list contents ────────────────────────────────────────────────────
" Matches note tokens (e.g. c, d#, e4, f#5) and rests (_) inside [ ].
" We define a region so the match is scoped and doesn't fire on lone
" single-letter identifiers outside a phrase.
syntax region patchlangNoteList
      \ start='\[' end='\]'
      \ contains=patchlangNote,patchlangRest,patchlangNoteListBracket

syntax match patchlangNote '\<[a-g][#b]\?[0-9]\?\>'
      \ contained
highlight default link patchlangNote Constant

syntax match patchlangRest '\<_\>'
      \ contained
highlight default link patchlangRest Comment

syntax match patchlangNoteListBracket '[\[\]]'
      \ contained
highlight default link patchlangNoteListBracket Delimiter

let b:current_syntax = 'patchlang'
