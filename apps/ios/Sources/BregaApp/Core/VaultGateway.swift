import Foundation
import BregaSwift

/// Process-wide accessor that opens the **App Group** vault for contexts that
/// don't own an `AppState` — App Intents, the Share Extension, and any other
/// extension process.
///
/// The vault is opened with the master password cached in the Keychain by the
/// main app's biometric flow (`KeychainService.saveMasterPassword`). Accessing
/// the cached password triggers a biometric / passcode check, after which the
/// real `BregaCoreClient` is returned and cached for the lifetime of the
/// process. If no password is cached (the user never enabled biometric unlock)
/// the gateway throws ``VaultGatewayError/locked`` so the caller can surface a
/// "unlock the app first" message instead of silently using mock data.
actor VaultGateway {

    static let shared = VaultGateway()

    private var cachedClient: BregaCoreClient?

    /// Returns a live `CoreClient` backed by the shared App Group vault,
    /// opening it on first use.
    func client() async throws -> any CoreClient {
        if let cachedClient { return cachedClient }

        let password: String
        let secretKey: String
        do {
            password = try KeychainService.loadMasterPassword(
                reason: "Unlock your brega vault"
            )
            secretKey = try KeychainService.loadSecretKey()
        } catch {
            throw VaultGatewayError.locked
        }

        do {
            let workspace = try await BregaWorkspace.open(
                path: AppGroup.vaultPath(), password: Data(password.utf8), secretKey: secretKey
            )
            let client = BregaCoreClient(workspace: workspace)
            cachedClient = client
            return client
        } catch let bregaError as BregaError {
            switch bregaError {
            case .VaultNotFound:
                throw VaultGatewayError.noVault
            case .InvalidCredentials:
                throw VaultGatewayError.locked
            default:
                throw bregaError
            }
        }
    }

    /// Drop the cached client (e.g. after the main app locks the vault).
    func invalidate() {
        cachedClient = nil
    }
}

/// Errors surfaced when an extension context cannot reach the vault.
enum VaultGatewayError: LocalizedError {
    case locked
    case noVault

    var errorDescription: String? {
        switch self {
        case .locked:
            return "Open brega and enable biometric unlock to use this from here."
        case .noVault:
            return "No brega vault found. Open the app to create one."
        }
    }
}
