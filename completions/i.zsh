#compdef i

autoload -U is-at-least

_i() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
'*::task:_default' \
&& ret=0
}

(( $+functions[_i_commands] )) ||
_i_commands() {
    local commands; commands=()
    _describe -t commands 'i commands' commands "$@"
}

if [ "$funcstack[1]" = "_i" ]; then
    _i "$@"
else
    compdef _i i
fi
