using System.Collections;
using UnityEngine;

public class SplashPlayer : MonoBehaviour
{
	private IEnumerator Start()
	{
        UnityEngine.Object.Instantiate<GameObject>(this.singletonSpawnerPrefab);
		while (!SingletonSpawner.SpawnDone)
		{
			yield return null;
		}
		this.StartSplash();
		yield break;
	}

	private void StartSplash()
	{
		string arg;
		if (Singleton<BuildCustomizationLoader>.Instance.IsChina)
		{
			string currentLocale = Singleton<Localizer>.Instance.CurrentLocale;
			if (Singleton<BuildCustomizationLoader>.Instance.CustomerID == "chinatelecom" || Singleton<BuildCustomizationLoader>.Instance.CustomerID == "chinamobile")
			{
				if (currentLocale == "zh-CN")
				{
					MonoBehaviour.print("SplashSequence_China_CN");
				}
				else
				{
					MonoBehaviour.print("SplashSequence_China");
				}
			}
			if (currentLocale == "zh-CN")
			{
				MonoBehaviour.print("SplashSequence_Talkweb_CN");
				arg = "Talkweb_CN";
			}
			else
			{
				MonoBehaviour.print("SplashSequence_PC-OSX ");
				arg = "PC-OSX";
			}
		}
		else if (Singleton<BuildCustomizationLoader>.Instance.IsHDVersion)
		{
			if (DeviceInfo.ActiveDeviceFamily == DeviceInfo.DeviceFamily.Ios || DeviceInfo.ActiveDeviceFamily == DeviceInfo.DeviceFamily.Android || DeviceInfo.ActiveDeviceFamily == DeviceInfo.DeviceFamily.BB10)
			{
				arg = "iPad";
			}
			else
			{
				arg = "PC-OSX";
			}
		}
		else
		{
			arg = "iPhone";
		}
		SplashScreenSequence original = Resources.Load<SplashScreenSequence>(string.Format("Splashes/Sequences/SplashSequence_{0}", arg));
		UnityEngine.Object.Instantiate<SplashScreenSequence>(original);
	}

	[SerializeField]
	private GameObject singletonSpawnerPrefab;
}
