vim.o.expandtab = true
vim.o.shiftwidth = 2
vim.o.tabstop = 2

fixers = {'prettier'}
languages = {'typescriptreact', 'css', 'html'}

for _, l in pairs(languages) do
	vim.g.ale_fixers[l] = fixers
end
