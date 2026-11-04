using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using ComFileTime = System.Runtime.InteropServices.ComTypes.FILETIME;

namespace ReforgerModSync.Core.Credentials;

/// <summary>Windows-Anmeldeinformationsspeicher: vom Betriebssystem verschlüsselt, pro Benutzer.</summary>
public sealed class WindowsCredentialStore : ICredentialStore
{
    private const uint CredTypeGeneric = 1;
    private const uint CredPersistLocalMachine = 2;
    private const int ErrorNotFound = 1168;

    public static string TargetName(string serverAddress, string profileId) =>
        $"ReforgerModSync:Server:{serverAddress.Trim()}:{profileId}";

    public string? ReadPassword(string serverAddress, string profileId)
    {
        if (!CredRead(TargetName(serverAddress, profileId), CredTypeGeneric, 0, out var handle))
        {
            var error = Marshal.GetLastWin32Error();
            return error == ErrorNotFound ? null : throw new Win32Exception(error);
        }

        try
        {
            var credential = Marshal.PtrToStructure<NativeCredential>(handle);
            return credential.CredentialBlob == IntPtr.Zero
                ? ""
                : Marshal.PtrToStringUni(credential.CredentialBlob, (int)credential.CredentialBlobSize / sizeof(char));
        }
        finally
        {
            CredFree(handle);
        }
    }

    public void SavePassword(string serverAddress, string profileId, string password)
    {
        var blob = Encoding.Unicode.GetBytes(password);
        var blobHandle = Marshal.AllocHGlobal(Math.Max(blob.Length, 1));
        try
        {
            Marshal.Copy(blob, 0, blobHandle, blob.Length);
            var credential = new NativeCredential
            {
                Type = CredTypeGeneric,
                TargetName = TargetName(serverAddress, profileId),
                UserName = "ReforgerModSync",
                CredentialBlob = blobHandle,
                CredentialBlobSize = (uint)blob.Length,
                Persist = CredPersistLocalMachine,
            };

            if (!CredWrite(ref credential, 0))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
        }
        finally
        {
            Marshal.Copy(new byte[blob.Length], 0, blobHandle, blob.Length);
            Marshal.FreeHGlobal(blobHandle);
            Array.Clear(blob);
        }
    }

    public bool DeletePassword(string serverAddress, string profileId)
    {
        if (CredDelete(TargetName(serverAddress, profileId), CredTypeGeneric, 0))
        {
            return true;
        }

        var error = Marshal.GetLastWin32Error();
        return error == ErrorNotFound ? false : throw new Win32Exception(error);
    }

    [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredRead(string target, uint type, uint flags, out IntPtr credential);

    [DllImport("advapi32.dll", EntryPoint = "CredWriteW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredWrite(ref NativeCredential credential, uint flags);

    [DllImport("advapi32.dll", EntryPoint = "CredDeleteW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CredDelete(string target, uint type, uint flags);

    [DllImport("advapi32.dll")]
    private static extern void CredFree(IntPtr buffer);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct NativeCredential
    {
        public uint Flags;
        public uint Type;
        public string? TargetName;
        public string? Comment;
        public ComFileTime LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public IntPtr Attributes;
        public string? TargetAlias;
        public string? UserName;
    }
}
