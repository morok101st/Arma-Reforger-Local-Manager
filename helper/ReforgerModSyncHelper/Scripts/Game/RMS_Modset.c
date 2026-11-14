//! Modus -rmsModset: laedt Mods aus einer Liste, ohne Server-Eintrag. Fuer aus ARMM importierte Server.
//! Die Liste steht in $profile:rms_modset.txt, je Zeile "id|version|name|get" (laden) oder "|have" (nur Anker).
//!
//! Eine Mod wird gefunden ueber (A) den lokalen Zwischenspeicher im Download-Ordner, (C) die Abhaengigkeiten
//! bereits gefundener Mods oder (B) die Workshop-Suche nach ihrem Namen mit Abgleich der ID. Nicht gelistete
//! Mods findet nur A oder C. Geprueft im Spike vom 2026-11-14 (91 von 91 Mods ohne Server).

class RMS_ModsetSearchParams : PageParams
{
	string m_sSearch;

	override void OnPack()
	{
		StoreString("search", m_sSearch);
		StoreBoolean("reported", false);
		StoreBoolean("hidden", false);
	}
}

enum RMS_EModsetState
{
	WAIT_MANAGER,
	RESOLVE,
	WAIT_PAGE,
	WAIT_DETAILS,
	WAIT_DEPENDENCIES,
	DOWNLOAD_NEXT,
	DOWNLOADING,
	FINISHED
}

class RMS_ModsetDownloader
{
	protected const float MANAGER_WAIT_SECONDS = 15;
	protected const float BACKEND_WAIT_SECONDS = 30;
	protected const float DEPENDENCY_WAIT_SECONDS = 15;

	protected ref array<string> m_aIds = {};
	protected ref array<string> m_aVersions = {};
	protected ref array<string> m_aNames = {};
	protected ref array<bool> m_aWanted = {};
	protected ref array<bool> m_aSearched = {};
	protected ref array<ref SCR_WorkshopItem> m_aItems = {};
	protected ref map<string, ref SCR_WorkshopItem> m_mDependencies = new map<string, ref SCR_WorkshopItem>();

	protected ref BackendCallback m_SearchCallback;
	protected ref RMS_ModsetSearchParams m_SearchParams;
	protected ref SCR_WorkshopItem m_Item;
	protected ref SCR_WorkshopItemActionDownload m_Action;

	protected RMS_EModsetState m_eState = RMS_EModsetState.WAIT_MANAGER;
	protected int m_iCursor;
	protected int m_iResolvedThisRound;
	protected bool m_bUseAnchors;
	protected int m_iDownload = -1;
	protected int m_iFailed;
	protected float m_fWait;
	protected float m_fLogTimer;

	void RMS_ModsetDownloader()
	{
		string path = "$profile:rms_modset.txt";
		if (FileIO.FileExists(path))
		{
			FileHandle file = FileIO.OpenFile(path, FileMode.READ);
			string line;
			while (file && file.ReadLine(line) >= 0)
			{
				array<string> parts = {};
				line.Split("|", parts, false);
				if (parts.Count() != 4)
					continue;

				m_aIds.Insert(parts[0]);
				m_aVersions.Insert(parts[1]);
				m_aNames.Insert(parts[2]);
				m_aWanted.Insert(parts[3] == "get");
				m_aSearched.Insert(false);
				m_aItems.Insert(null);
			}
			if (file)
				file.Close();
		}

		int wanted;
		foreach (bool get : m_aWanted)
		{
			if (get)
				wanted++;
		}
		Log(string.Format("MODSET_START mods=%1 get=%2", m_aIds.Count(), wanted));
	}

	protected void Log(string message)
	{
		Print("[RMS] " + message, LogLevel.NORMAL);
	}

	protected void Fail(int index, string reason)
	{
		Log(string.Format("MOD_FAILED %1 %2", m_aIds[index], reason));
		m_iFailed++;
	}

	void Tick(float timeslice)
	{
		m_fWait += timeslice;
		switch (m_eState)
		{
			case RMS_EModsetState.WAIT_MANAGER:
			{
				SCR_AddonManager manager = SCR_AddonManager.GetInstance();
				if (!manager)
					break;

				// Fehlen lokal Mods, kann die Pruefung des Spiels ausbleiben. Fuer die Suche wird sie nicht gebraucht.
				if (manager.GetReady() || m_fWait > MANAGER_WAIT_SECONDS)
				{
					Log(string.Format("MANAGER_READY ready=%1", manager.GetReady()));
					StartRound();
				}
				break;
			}

			case RMS_EModsetState.RESOLVE:
			{
				Resolve();
				break;
			}

			case RMS_EModsetState.WAIT_PAGE:
			{
				if (m_fWait > BACKEND_WAIT_SECONDS)
				{
					Log(string.Format("SEARCH_TIMEOUT %1", m_aIds[m_iCursor]));
					Advance();
				}
				break;
			}

			case RMS_EModsetState.WAIT_DETAILS:
			{
				if (m_Item.GetRevisionsLoaded())
				{
					m_aItems[m_iCursor] = m_Item;
					m_iResolvedThisRound++;
					m_fWait = 0;
					m_eState = RMS_EModsetState.WAIT_DEPENDENCIES;
				}
				else if (m_fWait > BACKEND_WAIT_SECONDS)
				{
					Log(string.Format("DETAILS_TIMEOUT %1", m_aIds[m_iCursor]));
					Advance();
				}
				break;
			}

			case RMS_EModsetState.WAIT_DEPENDENCIES:
			{
				if (m_Item.GetDependenciesLoaded() || m_fWait > DEPENDENCY_WAIT_SECONDS)
				{
					RememberDependencies(m_Item);
					Advance();
				}
				break;
			}

			case RMS_EModsetState.DOWNLOAD_NEXT:
			{
				StartNextDownload();
				break;
			}

			case RMS_EModsetState.DOWNLOADING:
			{
				if (m_Action.IsFailed() || m_Action.IsCanceled())
				{
					Fail(m_iDownload, "download failed");
					m_eState = RMS_EModsetState.DOWNLOAD_NEXT;
					break;
				}

				if (m_Action.IsCompleted())
				{
					Log(string.Format("MOD_DONE %1 %2", m_aIds[m_iDownload], m_aVersions[m_iDownload]));
					m_eState = RMS_EModsetState.DOWNLOAD_NEXT;
					break;
				}

				m_fLogTimer += timeslice;
				if (m_fLogTimer > 5)
				{
					m_fLogTimer = 0;
					Log(string.Format("PROGRESS %1 running=%2", m_aIds[m_iDownload], m_Item.IsDownloadRunning()));
				}
				break;
			}
		}
	}

	// --- Finden ---

	protected void StartRound()
	{
		m_iCursor = 0;
		m_iResolvedThisRound = 0;
		m_fWait = 0;
		m_eState = RMS_EModsetState.RESOLVE;
	}

	protected void Advance()
	{
		m_iCursor++;
		m_fWait = 0;
		m_eState = RMS_EModsetState.RESOLVE;
	}

	//! Sucht die naechste noch offene Mod. Anker (nicht zu ladende Mods) kommen erst dran, wenn ohne sie
	//! etwas offen bliebe. Eine Runde wiederholt sich, solange sie etwas Neues gefunden hat.
	protected void Resolve()
	{
		while (m_iCursor < m_aIds.Count())
		{
			int index = m_iCursor;
			if (m_aItems[index] || (!m_aWanted[index] && !m_bUseAnchors))
			{
				m_iCursor++;
				continue;
			}

			WorkshopItem local = GetGame().GetBackendApi().GetWorkshop().FindItem(m_aIds[index]);
			if (local)
			{
				Log(string.Format("RESOLVED %1 via=cache", m_aIds[index]));
				UseItem(SCR_AddonManager.GetInstance().Register(local));
				return;
			}

			SCR_WorkshopItem dependency = m_mDependencies.Get(m_aIds[index]);
			if (dependency)
			{
				Log(string.Format("RESOLVED %1 via=dependency", m_aIds[index]));
				UseItem(dependency);
				return;
			}

			if (!m_aSearched[index])
			{
				m_aSearched[index] = true;
				Search(index);
				return;
			}

			m_iCursor++;
		}

		if (m_iResolvedThisRound > 0)
		{
			StartRound();
			return;
		}

		if (!m_bUseAnchors && AnyWantedOpen())
		{
			Log("ANCHORS");
			m_bUseAnchors = true;
			StartRound();
			return;
		}

		for (int index = 0; index < m_aIds.Count(); index++)
		{
			if (m_aWanted[index] && !m_aItems[index])
				Fail(index, "not_found");
		}
		m_eState = RMS_EModsetState.DOWNLOAD_NEXT;
	}

	protected bool AnyWantedOpen()
	{
		for (int index = 0; index < m_aIds.Count(); index++)
		{
			if (m_aWanted[index] && !m_aItems[index])
				return true;
		}
		return false;
	}

	protected void Search(int index)
	{
		m_SearchParams = new RMS_ModsetSearchParams();
		m_SearchParams.m_sSearch = m_aNames[index];
		m_SearchParams.limit = 50;
		m_SearchParams.offset = 0;

		m_SearchCallback = new BackendCallback();
		m_SearchCallback.SetOnSuccess(OnPageLoaded);
		m_SearchCallback.SetOnError(OnPageError);

		WorkshopApi api = GetGame().GetBackendApi().GetWorkshop();
		api.SetPageSize(50);
		m_fWait = 0;
		m_eState = RMS_EModsetState.WAIT_PAGE;
		api.RequestPage(m_SearchCallback, m_SearchParams, true);
	}

	protected void OnPageLoaded()
	{
		if (m_eState != RMS_EModsetState.WAIT_PAGE)
			return;

		array<WorkshopItem> items = {};
		GetGame().GetBackendApi().GetWorkshop().GetPageItems(items);
		foreach (WorkshopItem item : items)
		{
			if (item.Id() == m_aIds[m_iCursor])
			{
				Log(string.Format("RESOLVED %1 via=search", m_aIds[m_iCursor]));
				UseItem(SCR_AddonManager.GetInstance().Register(item));
				return;
			}
		}
		Advance();
	}

	protected void OnPageError(BackendCallback callback)
	{
		if (m_eState != RMS_EModsetState.WAIT_PAGE)
			return;

		Log(string.Format("SEARCH_FAILED %1 http=%2", m_aIds[m_iCursor], callback.GetHttpCode()));
		Advance();
	}

	protected void UseItem(SCR_WorkshopItem item)
	{
		m_Item = item;
		m_Item.LoadDetails();
		m_fWait = 0;
		m_eState = RMS_EModsetState.WAIT_DETAILS;
	}

	protected void RememberDependencies(SCR_WorkshopItem item)
	{
		array<ref SCR_WorkshopItem> dependencies = item.GetLatestDependencies();
		if (!dependencies)
			return;

		foreach (SCR_WorkshopItem dependency : dependencies)
		{
			if (!m_mDependencies.Contains(dependency.GetId()))
				m_mDependencies.Set(dependency.GetId(), dependency);
		}
	}

	// --- Laden ---

	protected void StartNextDownload()
	{
		m_iDownload++;
		while (m_iDownload < m_aIds.Count() && (!m_aWanted[m_iDownload] || !m_aItems[m_iDownload]))
			m_iDownload++;

		if (m_iDownload >= m_aIds.Count())
		{
			Log(string.Format("DONE failed=%1", m_iFailed));
			m_eState = RMS_EModsetState.FINISHED;
			GetGame().RequestClose();
			return;
		}

		m_Item = m_aItems[m_iDownload];
		Revision revision = m_Item.FindRevision(m_aVersions[m_iDownload]);
		if (!revision)
		{
			Fail(m_iDownload, "version_unavailable");
			return;
		}

		m_Item.SetItemTargetRevision(revision);
		m_Action = m_Item.Download(revision);
		if (!m_Action)
		{
			Fail(m_iDownload, "download could not be started");
			return;
		}

		m_Action.Activate();
		m_fLogTimer = 0;
		m_eState = RMS_EModsetState.DOWNLOADING;
		Log(string.Format("DOWNLOAD_STARTED %1 %2 %3", m_aIds[m_iDownload], m_aVersions[m_iDownload], m_aNames[m_iDownload]));
	}
}
