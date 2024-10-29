
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'sk' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'sk'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'sk' {
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'Comma-separated list of sort criteria to apply when the scores are tied.')
            [CompletionResult]::new('--tiebreak', '--tiebreak', [CompletionResultType]::ParameterName, 'Comma-separated list of sort criteria to apply when the scores are tied.')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Fields to be matched')
            [CompletionResult]::new('--nth', '--nth', [CompletionResultType]::ParameterName, 'Fields to be matched')
            [CompletionResult]::new('--with-nth', '--with-nth', [CompletionResultType]::ParameterName, 'Fields to be transformed')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Delimiter between fields')
            [CompletionResult]::new('--delimiter', '--delimiter', [CompletionResultType]::ParameterName, 'Delimiter between fields')
            [CompletionResult]::new('--algo', '--algo', [CompletionResultType]::ParameterName, 'Fuzzy matching algorithm')
            [CompletionResult]::new('--case', '--case', [CompletionResultType]::ParameterName, 'Case sensitivity')
            [CompletionResult]::new('-b', '-b', [CompletionResultType]::ParameterName, 'Comma separated list of bindings')
            [CompletionResult]::new('--bind', '--bind', [CompletionResultType]::ParameterName, 'Comma separated list of bindings')
            [CompletionResult]::new('-c', '-c', [CompletionResultType]::ParameterName, 'Command to invoke dynamically in interactive mode')
            [CompletionResult]::new('--cmd', '--cmd', [CompletionResultType]::ParameterName, 'Command to invoke dynamically in interactive mode')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Set color theme')
            [CompletionResult]::new('--skip-to-pattern', '--skip-to-pattern', [CompletionResultType]::ParameterName, 'Show the matched pattern at the line start')
            [CompletionResult]::new('--layout', '--layout', [CompletionResultType]::ParameterName, 'Set layout')
            [CompletionResult]::new('--height', '--height', [CompletionResultType]::ParameterName, 'Height of skim''s window')
            [CompletionResult]::new('--min-height', '--min-height', [CompletionResultType]::ParameterName, 'Minimum height of skim''s window')
            [CompletionResult]::new('--margin', '--margin', [CompletionResultType]::ParameterName, 'Screen margin')
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'Set prompt')
            [CompletionResult]::new('--prompt', '--prompt', [CompletionResultType]::ParameterName, 'Set prompt')
            [CompletionResult]::new('--cmd-prompt', '--cmd-prompt', [CompletionResultType]::ParameterName, 'Set prompt in command mode')
            [CompletionResult]::new('--tabstop', '--tabstop', [CompletionResultType]::ParameterName, 'Number of spaces that make up a tab')
            [CompletionResult]::new('--header', '--header', [CompletionResultType]::ParameterName, 'Set header, displayed next to the info')
            [CompletionResult]::new('--header-lines', '--header-lines', [CompletionResultType]::ParameterName, 'Number of lines of the input treated as header')
            [CompletionResult]::new('--history', '--history', [CompletionResultType]::ParameterName, 'History file')
            [CompletionResult]::new('--history-size', '--history-size', [CompletionResultType]::ParameterName, 'Maximum number of query history entries to keep')
            [CompletionResult]::new('--cmd-history', '--cmd-history', [CompletionResultType]::ParameterName, 'Command history file')
            [CompletionResult]::new('--cmd-history-size', '--cmd-history-size', [CompletionResultType]::ParameterName, 'Maximum number of query history entries to keep')
            [CompletionResult]::new('--preview', '--preview', [CompletionResultType]::ParameterName, 'Preview command')
            [CompletionResult]::new('--preview-window', '--preview-window', [CompletionResultType]::ParameterName, 'Preview window layout')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Initial query')
            [CompletionResult]::new('--query', '--query', [CompletionResultType]::ParameterName, 'Initial query')
            [CompletionResult]::new('--cmd-query', '--cmd-query', [CompletionResultType]::ParameterName, 'Initial query in interactive mode')
            [CompletionResult]::new('--expect', '--expect', [CompletionResultType]::ParameterName, 'Comma separated list of keys used to complete skim')
            [CompletionResult]::new('--pre-select-n', '--pre-select-n', [CompletionResultType]::ParameterName, 'Pre-select the first n items in multi-selection mode')
            [CompletionResult]::new('--pre-select-pat', '--pre-select-pat', [CompletionResultType]::ParameterName, 'Pre-select the matched items in multi-selection mode')
            [CompletionResult]::new('--pre-select-items', '--pre-select-items', [CompletionResultType]::ParameterName, 'Pre-select the items separated by newline character')
            [CompletionResult]::new('--pre-select-file', '--pre-select-file', [CompletionResultType]::ParameterName, 'Pre-select the items read from this file')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Query for filter mode')
            [CompletionResult]::new('--filter', '--filter', [CompletionResultType]::ParameterName, 'Query for filter mode')
            [CompletionResult]::new('--hscroll-off', '--hscroll-off', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--jump-labels', '--jump-labels', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--tac', '--tac', [CompletionResultType]::ParameterName, 'Show results in reverse order')
            [CompletionResult]::new('--no-sort', '--no-sort', [CompletionResultType]::ParameterName, 'Do not sort the results')
            [CompletionResult]::new('-e', '-e', [CompletionResultType]::ParameterName, 'Run in exact mode')
            [CompletionResult]::new('--exact', '--exact', [CompletionResultType]::ParameterName, 'Run in exact mode')
            [CompletionResult]::new('--regex', '--regex', [CompletionResultType]::ParameterName, 'Start in regex mode instead of fuzzy-match')
            [CompletionResult]::new('-m', '-m', [CompletionResultType]::ParameterName, 'Enable multiple selection')
            [CompletionResult]::new('--multi', '--multi', [CompletionResultType]::ParameterName, 'Enable multiple selection')
            [CompletionResult]::new('--no-multi', '--no-multi', [CompletionResultType]::ParameterName, 'Disable multiple selection')
            [CompletionResult]::new('--no-mouse', '--no-mouse', [CompletionResultType]::ParameterName, 'Disable mouse')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'Run in interactive mode')
            [CompletionResult]::new('--interactive', '--interactive', [CompletionResultType]::ParameterName, 'Run in interactive mode')
            [CompletionResult]::new('--no-hscroll', '--no-hscroll', [CompletionResultType]::ParameterName, 'Disable horizontal scroll')
            [CompletionResult]::new('--keep-right', '--keep-right', [CompletionResultType]::ParameterName, 'Keep the right end of the line visible on overflow')
            [CompletionResult]::new('--no-clear-if-empty', '--no-clear-if-empty', [CompletionResultType]::ParameterName, 'Do not clear previous line if the command returns an empty result')
            [CompletionResult]::new('--no-clear-start', '--no-clear-start', [CompletionResultType]::ParameterName, 'Do not clear items on start')
            [CompletionResult]::new('--no-clear', '--no-clear', [CompletionResultType]::ParameterName, 'Do not clear screen on exit')
            [CompletionResult]::new('--show-cmd-error', '--show-cmd-error', [CompletionResultType]::ParameterName, 'Show error message if command fails')
            [CompletionResult]::new('--reverse', '--reverse', [CompletionResultType]::ParameterName, 'Shorthand for reverse layout')
            [CompletionResult]::new('--no-height', '--no-height', [CompletionResultType]::ParameterName, 'Disable height feature')
            [CompletionResult]::new('--ansi', '--ansi', [CompletionResultType]::ParameterName, 'Parse ANSI color codes in input strings')
            [CompletionResult]::new('--inline-info', '--inline-info', [CompletionResultType]::ParameterName, 'Display info next to the query')
            [CompletionResult]::new('--read0', '--read0', [CompletionResultType]::ParameterName, 'Read input delimited by ASCII NUL(\\0) characters')
            [CompletionResult]::new('--print0', '--print0', [CompletionResultType]::ParameterName, 'Print output delimited by ASCII NUL(\\0) characters')
            [CompletionResult]::new('--print-query', '--print-query', [CompletionResultType]::ParameterName, 'Print the query as the first line')
            [CompletionResult]::new('--print-cmd', '--print-cmd', [CompletionResultType]::ParameterName, 'Print the command as the first line (after print-query)')
            [CompletionResult]::new('--print-score', '--print-score', [CompletionResultType]::ParameterName, 'Print the command as the first line (after print-cmd)')
            [CompletionResult]::new('-1', '-1', [CompletionResultType]::ParameterName, 'Automatically select the match if there is only one')
            [CompletionResult]::new('--select-1', '--select-1', [CompletionResultType]::ParameterName, 'Automatically select the match if there is only one')
            [CompletionResult]::new('-0', '-0', [CompletionResultType]::ParameterName, 'Automatically exit when no match is left')
            [CompletionResult]::new('--exit-0', '--exit-0', [CompletionResultType]::ParameterName, 'Automatically exit when no match is left')
            [CompletionResult]::new('--sync', '--sync', [CompletionResultType]::ParameterName, 'Synchronous search for multi-staged filtering')
            [CompletionResult]::new('-x', '-x', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--extended', '--extended', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--literal', '--literal', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--cycle', '--cycle', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--filepath-word', '--filepath-word', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--border', '--border', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--no-bold', '--no-bold', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--info', '--info', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--pointer', '--pointer', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--marker', '--marker', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('--phony', '--phony', [CompletionResultType]::ParameterName, 'Reserved for later use')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
