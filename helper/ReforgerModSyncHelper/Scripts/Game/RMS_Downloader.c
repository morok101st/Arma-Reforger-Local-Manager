//! Laedt beim Start des Clients die per -rmsMods "id:version;id:version" uebergebenen Mods.
//! Ohne diesen Startparameter macht die Mod nichts. Status steht mit Praefix [RMS] im console.log.

class RMS_SearchParams : PageParams
{
	string m_sSearch;

	override void OnPack()
	{
		StoreString("search", m_sSearch);
		StoreBoolean("reported", false);
		StoreBoolean("hidden", false);
	}
}

enum RMS_EState
{
	WAIT_MANAGER,
	NEXT_MOD,
	WAIT_PAGE,
	WAIT_DETAILS,
	DOWNLOADING,
	FINISHED
}

class RMS_Downloader
{
	protected ref array<string> m_aIds = {};
	protected ref array<string> m_aVersions = {};
	protected ref BackendCallback m_Callback;
	protected ref RMS_SearchParams m_Params;
	protected ref SCR_WorkshopItem m_Item;
	protected ref SCR_WorkshopItemActionDownload m_Action;
	protected RMS_EState m_eState = RMS_EState.WAIT_MANAGER;
	protected int m_iIndex = -1;
	protected int m_iFailed;
	protected float m_fLogTimer;
	protected float m_fManagerWait;
	protected const float MANAGER_WAIT_SECONDS = 15;

	void RMS_Downloader(string mods)
	{
		array<string> entries = {};
		mods.Split(";", entries, true);
		foreach (string entry : entries)
		{
			array<string> parts = {};
			entry.Split(":", parts, true);
			if (parts.Count() >= 1)
			{
				m_aIds.Insert(parts[0]);
				// Ohne Version wird die neueste geladen.
				if (parts.Count() == 2)
					m_aVersions.Insert(parts[1]);
				else
					m_aVersions.Insert(string.Empty);
			}
		}

		Log(string.Format("START mods=%1", m_aIds.Count()));
	}

	protected void Log(string message)
	{
		Print("[RMS] " + message, LogLevel.NORMAL);
	}

	protected void Fail(string message)
	{
		Log(string.Format("MOD_FAILED %1 %2", m_aIds[m_iIndex], message));
		m_iFailed++;
		m_eState = RMS_EState.NEXT_MOD;
	}

	void Tick(float timeslice)
	{
		switch (m_eState)
		{
			case RMS_EState.WAIT_MANAGER:
			{
				SCR_AddonManager manager = SCR_AddonManager.GetInstance();
				if (!manager)
					break;

				if (manager.GetReady())
				{
					Log("MANAGER_READY");
					m_eState = RMS_EState.NEXT_MOD;
					break;
				}

				// Fehlen lokal Mods, kann die Pruefung des Spiels ausbleiben (AddonCheckError wird dort
				// nicht behandelt). Fuer den Download selbst wird ihr Ergebnis nicht gebraucht.
				m_fManagerWait += timeslice;
				if (m_fManagerWait > MANAGER_WAIT_SECONDS)
				{
					Log("MANAGER_TIMEOUT");
					m_eState = RMS_EState.NEXT_MOD;
				}
				break;
			}

			case RMS_EState.NEXT_MOD:
			{
				m_iIndex++;
				if (m_iIndex >= m_aIds.Count())
				{
					Log(string.Format("DONE failed=%1", m_iFailed));
					m_eState = RMS_EState.FINISHED;
					GetGame().RequestClose();
					break;
				}
				RequestItem();
				break;
			}

			case RMS_EState.WAIT_DETAILS:
			{
				if (m_Item.GetRevisionsLoaded())
					StartDownload();
				break;
			}

			case RMS_EState.DOWNLOADING:
			{
				if (m_Action.IsFailed() || m_Action.IsCanceled())
				{
					Fail("download failed");
					break;
				}

				Revision local = m_Item.GetCurrentLocalRevision();
				string wanted = m_aVersions[m_iIndex];
				if (local && (wanted.IsEmpty() || local.GetVersion() == wanted) && !m_Item.IsDownloadRunning())
				{
					Log(string.Format("MOD_DONE %1 %2", m_aIds[m_iIndex], m_aVersions[m_iIndex]));
					m_eState = RMS_EState.NEXT_MOD;
					break;
				}

				m_fLogTimer += timeslice;
				if (m_fLogTimer > 5)
				{
					m_fLogTimer = 0;
					Log(string.Format("PROGRESS %1 running=%2", m_aIds[m_iIndex], m_Item.IsDownloadRunning()));
				}
				break;
			}
		}
	}

	protected void RequestItem()
	{
		Log(string.Format("SEARCH %1", m_aIds[m_iIndex]));
		m_Params = new RMS_SearchParams();
		m_Params.m_sSearch = m_aIds[m_iIndex];
		m_Params.limit = 20;
		m_Params.offset = 0;

		m_Callback = new BackendCallback();
		m_Callback.SetOnSuccess(OnPageLoaded);
		m_Callback.SetOnError(OnPageError);

		WorkshopApi api = GetGame().GetBackendApi().GetWorkshop();
		api.SetPageSize(20);
		m_eState = RMS_EState.WAIT_PAGE;
		api.RequestPage(m_Callback, m_Params, true);
	}

	protected void OnPageLoaded()
	{
		WorkshopApi api = GetGame().GetBackendApi().GetWorkshop();
		array<WorkshopItem> items = {};
		api.GetPageItems(items);
		Log(string.Format("PAGE items=%1", items.Count()));

		foreach (WorkshopItem item : items)
		{
			if (item.Id() == m_aIds[m_iIndex])
			{
				m_Item = SCR_AddonManager.GetInstance().Register(item);
				m_Item.LoadDetails();
				m_eState = RMS_EState.WAIT_DETAILS;
				Log(string.Format("FOUND %1 %2", item.Id(), item.Name()));
				return;
			}
		}

		Fail("not found");
	}

	protected void OnPageError()
	{
		Fail("page request failed");
	}

	protected void StartDownload()
	{
		if (m_aVersions[m_iIndex].IsEmpty())
		{
			m_Action = m_Item.DownloadLatestVersion();
		}
		else
		{
			Revision revision = m_Item.FindRevision(m_aVersions[m_iIndex]);
			if (!revision)
			{
				Fail(string.Format("version %1 not available", m_aVersions[m_iIndex]));
				return;
			}

			m_Action = m_Item.Download(revision);
		}

		if (!m_Action)
		{
			Fail("download could not be started");
			return;
		}

		m_Action.Activate();
		m_eState = RMS_EState.DOWNLOADING;
		Log(string.Format("DOWNLOAD_STARTED %1 %2", m_aIds[m_iIndex], m_aVersions[m_iIndex]));
	}
}

//! Holt die Mod-Liste eines Servers ueber seinen Eintrag im Backend, wie der Server-Browser.
//! Es entsteht keine Verbindung zum Spielserver und kein Beitritt.
class RMS_RoomReader
{
	protected string m_sAddress;
	protected ref FilteredServerParams m_Params;
	protected ref BackendCallback m_SearchCallback;
	protected ref BackendCallback m_PasswordCallback;
	protected ref BackendCallback m_ListCallback;
	protected ref BackendCallback m_JoinCallback;
	protected ref RoomJoinData m_JoinData;
	protected ref array<Room> m_aRooms = {};
	protected Room m_Room;
	protected string m_sPassword;
	protected bool m_bJoin;
	protected bool m_bStarted;
	protected bool m_bFinished;

	//! @param join Dem Server danach beitreten, wie der Server-Browser, statt nur die Liste zu melden.
	void RMS_RoomReader(string address, bool join)
	{
		m_sAddress = address;
		m_bJoin = join;
	}

	protected void Log(string message)
	{
		Print("[RMS] " + message, LogLevel.NORMAL);
	}

	protected void Finish(string result)
	{
		Log(result);
		m_bFinished = true;

		// Beim Beitritt bleibt das Spiel im Fehlerfall offen, der Spieler kann dann selbst weitermachen.
		if (!m_bJoin)
			GetGame().RequestClose();
	}

	void Tick()
	{
		if (m_bStarted || m_bFinished)
			return;

		// Der Server-Browser kann erst im Hauptmenue suchen.
		if (!SCR_AddonManager.GetInstance())
			return;

		m_bStarted = true;
		Log(string.Format("ROOM_SEARCH %1", m_sAddress));

		m_Params = new FilteredServerParams();
		m_Params.SetHostAddress(m_sAddress);
		m_Params.SetUsePlayerLimit(false);

		m_SearchCallback = new BackendCallback();
		m_SearchCallback.SetOnSuccess(OnSearchSuccess);
		m_SearchCallback.SetOnError(OnSearchError);
		GetGame().GetBackendApi().GetClientLobby().SearchTarget(m_Params, m_SearchCallback);
	}

	protected void OnSearchSuccess()
	{
		GetGame().GetBackendApi().GetClientLobby().Target(m_aRooms);
		if (m_aRooms.IsEmpty())
		{
			Finish("ROOM_NOT_FOUND");
			return;
		}

		m_Room = m_aRooms[0];
		Log(string.Format("ROOM_FOUND rooms=%1 name=%2 protected=%3 authorized=%4", m_aRooms.Count(), m_Room.Name(), m_Room.PasswordProtected(), m_Room.IsAuthorized()));

		m_sPassword = ReadPassword();
		if (m_Room.IsAuthorized())
		{
			LoadList();
			return;
		}

		if (m_sPassword.IsEmpty())
		{
			Finish("ROOM_PASSWORD_REQUIRED");
			return;
		}

		m_PasswordCallback = new BackendCallback();
		m_PasswordCallback.SetOnSuccess(LoadList);
		m_PasswordCallback.SetOnError(OnPasswordError);
		m_Room.VerifyPassword(m_sPassword, m_PasswordCallback);
	}

	//! Aus einer Datei und nicht per Startparameter, weil der Client seine Startparameter ins Log schreibt.
	protected string ReadPassword()
	{
		const string path = "$profile:rms_server_password.txt";
		if (!FileIO.FileExists(path))
			return string.Empty;

		string password;
		FileHandle file = FileIO.OpenFile(path, FileMode.READ);
		if (file)
		{
			file.ReadLine(password);
			file.Close();
		}

		FileIO.DeleteFile(path);
		return password;
	}

	protected void OnSearchError(BackendCallback callback)
	{
		Finish(string.Format("ROOM_SEARCH_FAILED http=%1", callback.GetHttpCode()));
	}

	protected void OnPasswordError(BackendCallback callback)
	{
		Finish(string.Format("ROOM_PASSWORD_REJECTED http=%1", callback.GetHttpCode()));
	}

	protected void LoadList()
	{
		Log("ROOM_AUTHORIZED");
		m_ListCallback = new BackendCallback();
		m_ListCallback.SetOnSuccess(OnListLoaded);
		m_ListCallback.SetOnError(OnListError);
		m_Room.LoadDownloadList(m_ListCallback);
	}

	protected void OnListError(BackendCallback callback)
	{
		Finish(string.Format("ROOM_LIST_FAILED http=%1", callback.GetHttpCode()));
	}

	protected void OnListLoaded()
	{
		array<Dependency> dependencies = {};
		m_Room.GetItems(dependencies);

		if (m_bJoin)
		{
			Join();
			return;
		}

		foreach (Dependency dependency : dependencies)
		{
			Log(string.Format("SERVER_MOD %1 %2 %3", dependency.GetID(), dependency.GetVersion(), dependency.GetName()));
		}

		Finish(string.Format("LIST_DONE mods=%1", dependencies.Count()));
	}

	//! Wie JoinProcess_Join im Server-Browser. Das Spiel laedt danach die Mods des Servers neu und verbindet sich.
	protected void Join()
	{
		Log("JOIN_REQUEST");
		m_JoinData = new RoomJoinData();
		m_JoinData.m_Password = m_sPassword;

		m_JoinCallback = new BackendCallback();
		m_JoinCallback.SetOnSuccess(OnJoinSuccess);
		m_JoinCallback.SetOnError(OnJoinError);
		m_Room.Join(m_JoinCallback, m_JoinData);
	}

	protected void OnJoinSuccess()
	{
		m_sPassword = string.Empty;
		Log("JOIN_ACCEPTED");

		ClientLobbyApi lobby = GetGame().GetBackendApi().GetClientLobby();
		if (!GameStateTransitions.RequestConnectViaRoom(lobby.GetJoinRoom()))
		{
			Finish("JOIN_TRANSITION_REFUSED");
			return;
		}

		GetGame().GetMenuManager().CloseAllMenus();
		GetGame().GetBackendApi().GetWorkshop().Cleanup();
		m_bFinished = true;
		Log("JOIN_CONNECTING");
	}

	protected void OnJoinError(BackendCallback callback)
	{
		m_sPassword = string.Empty;
		Finish(string.Format("JOIN_FAILED http=%1 api=%2", callback.GetHttpCode(), callback.GetApiCode()));
	}
}

modded class ArmaReforgerScripted
{
	protected ref RMS_Downloader m_RmsDownloader;
	protected ref RMS_RoomReader m_RmsRoomReader;
	protected bool m_bRmsChecked;

	override void OnUpdate(BaseWorld world, float timeslice)
	{
		super.OnUpdate(world, timeslice);

		if (!m_bRmsChecked)
		{
			m_bRmsChecked = true;
			string mods;
			if (System.GetCLIParam("rmsMods", mods))
				m_RmsDownloader = new RMS_Downloader(mods);

			string server;
			if (System.GetCLIParam("rmsServer", server))
				m_RmsRoomReader = new RMS_RoomReader(server, false);
			else if (System.GetCLIParam("rmsJoin", server))
				m_RmsRoomReader = new RMS_RoomReader(server, true);
		}

		if (m_RmsDownloader)
			m_RmsDownloader.Tick(timeslice);

		if (m_RmsRoomReader)
			m_RmsRoomReader.Tick();
	}
}
