-- Static map: starting state is stored in the Civ6Map file.
function InitializeNewGame() end
LuaEvents.NewGameInitialized.Add(InitializeNewGame);
