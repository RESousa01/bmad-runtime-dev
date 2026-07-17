using System.Net;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Sapphirus.DesktopSupportApi;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests;

public sealed class SupportApiSecurityTests
{
    private const string TenantId = "11111111-1111-1111-1111-111111111111";
    private const string ApprovedClientId = "22222222-2222-2222-2222-222222222222";
    private const string OtherClientId = "33333333-3333-3333-3333-333333333333";
    private const string SubjectA = "44444444-4444-4444-4444-444444444444";
    private const string SubjectB = "55555555-5555-5555-5555-555555555555";
    private const string ValidHash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    [Fact]
    public async Task Unauthenticated_request_is_challenged_instead_of_failing_rate_partition()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await client.GetAsync(
            "/desktop/v1/bootstrap",
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public void Authority_without_an_exact_tenant_segment_is_rejected()
    {
        SupportPlaneOptions options = new()
        {
            Authority = "https://login.microsoftonline.com/common",
            Audience = "api://66666666-6666-6666-6666-666666666666",
            ApprovedDesktopClientId = ApprovedClientId,
        };

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(new TestHostEnvironment()));
    }

    [Theory]
    [InlineData("https://user@login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0")]
    [InlineData("https://login.microsoftonline.com:444/11111111-1111-1111-1111-111111111111/v2.0")]
    [InlineData("https://login.microsoftonline.com:443/11111111-1111-1111-1111-111111111111/v2.0")]
    [InlineData("https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0?source=test")]
    [InlineData("https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0#fragment")]
    [InlineData("https://login.microsoftonline.com//11111111-1111-1111-1111-111111111111/v2.0")]
    [InlineData("https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0/")]
    [InlineData("https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2%2E0")]
    public void Authority_with_noncanonical_uri_components_is_rejected(string authority)
    {
        SupportPlaneOptions options = new()
        {
            Authority = authority,
            Audience = "api://66666666-6666-6666-6666-666666666666",
            ApprovedDesktopClientId = ApprovedClientId,
        };

        Assert.Throws<InvalidOperationException>(() =>
            options.Validate(new TestHostEnvironment()));
    }

    [Fact]
    public async Task Duplicate_top_level_json_members_are_rejected()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        string body = $$"""
            {
              "installationPublicKeyHash": "{{ValidHash}}",
              "clientRelease": "0.1.0-beta.1",
              "platform": "windows",
              "architecture": "x64",
              "architecture": "x64",
              "tenantPolicyVersion": "development-1"
            }
            """;

        using HttpResponseMessage response = await SendRawJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/devices/registrations",
            SubjectA,
            DesktopScopes.Access,
            body,
            "idem-duplicate-registration-member");

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        await AssertSafeProblemAsync(response, "request_invalid", HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task Casing_aliases_are_rejected_instead_of_binding_to_contract_members()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        string body = $$"""
            {
              "InstallationPublicKeyHash": "{{ValidHash}}",
              "ClientRelease": "0.1.0-beta.1",
              "Platform": "windows",
              "Architecture": "x64",
              "TenantPolicyVersion": "development-1"
            }
            """;

        using HttpResponseMessage response = await SendRawJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/devices/registrations",
            SubjectA,
            DesktopScopes.Access,
            body,
            "idem-registration-casing-alias");

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        await AssertSafeProblemAsync(response, "request_invalid", HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task Only_the_configured_desktop_client_is_authorized()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage denied = await SendAsync(
            client,
            HttpMethod.Get,
            "/desktop/v1/bootstrap",
            SubjectA,
            OtherClientId,
            DesktopScopes.Access);
        using HttpResponseMessage accepted = await SendAsync(
            client,
            HttpMethod.Get,
            "/desktop/v1/bootstrap",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);

        Assert.Equal(HttpStatusCode.Forbidden, denied.StatusCode);
        Assert.Equal(HttpStatusCode.OK, accepted.StatusCode);
    }

    [Fact]
    public async Task Registration_is_subject_bound_and_revocation_is_retained()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);

        using HttpResponseMessage activeLease = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-active-device");
        Assert.Equal(HttpStatusCode.OK, activeLease.StatusCode);

        using HttpResponseMessage crossSubjectLease = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectB,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-cross-subject");
        Assert.Equal(HttpStatusCode.Forbidden, crossSubjectLease.StatusCode);

        using HttpResponseMessage revoke = await SendAsync(
            client,
            HttpMethod.Delete,
            $"/desktop/v1/devices/registrations/{registration.RegistrationId}",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        Assert.Equal(HttpStatusCode.NoContent, revoke.StatusCode);

        using HttpResponseMessage revokedLease = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-revoked-device");
        Assert.Equal(HttpStatusCode.Forbidden, revokedLease.StatusCode);
    }

    [Fact]
    public async Task Device_commit_before_revocation_stales_the_operation_lease()
    {
        const string subject = "device-operation-subject";
        MemoryDeviceRegistry registry = new();
        DeviceRegistrationResponse registration = await registry.RegisterAsync(
            subject,
            new DeviceRegistrationRequest(
                Hash("device-operation-key"),
                "0.1.0-beta.1",
                "windows",
                "x64",
                "development-1"),
            TestContext.Current.CancellationToken);
        ActiveDeviceRegistration? operationLease = await registry.FindActiveAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken);
        Assert.NotNull(operationLease);
        SignedEntitlementLease lease = CreateSignedLease(
            subject,
            registration.RegistrationId);

        SignedEntitlementLease committed = await registry.CommitLeaseIfActiveAsync(
            operationLease!,
            lease,
            TestContext.Current.CancellationToken);
        DeviceRevocationOutcome revocation = await registry.RevokeAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken);

        Assert.Equal(lease, committed);
        Assert.Equal(DeviceRevocationOutcome.Revoked, revocation);
        await Assert.ThrowsAsync<DeviceRegistrationRevokedException>(() =>
            registry.CommitLeaseIfActiveAsync(
                operationLease!,
                lease,
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public void Device_authority_commit_contract_accepts_no_arbitrary_callback()
    {
        Assert.DoesNotContain(
            typeof(IDeviceRegistry).GetMethods().SelectMany(method => method.GetParameters()),
            parameter => typeof(Delegate).IsAssignableFrom(parameter.ParameterType));
    }

    [Fact]
    public async Task Device_operation_lease_cannot_cross_registry_authority()
    {
        const string subject = "cross-registry-subject";
        DeviceRegistrationRequest request = new(
            Hash("cross-registry-key"),
            "0.1.0-beta.1",
            "windows",
            "x64",
            "development-1");
        MemoryDeviceRegistry first = new();
        MemoryDeviceRegistry second = new();
        DeviceRegistrationResponse firstRegistration = await first.RegisterAsync(
            subject,
            request,
            TestContext.Current.CancellationToken);
        await second.RegisterAsync(subject, request, TestContext.Current.CancellationToken);
        ActiveDeviceRegistration? operationLease = await first.FindActiveAsync(
            subject,
            firstRegistration.RegistrationId,
            TestContext.Current.CancellationToken);
        Assert.NotNull(operationLease);

        await Assert.ThrowsAsync<DeviceRegistrationRevokedException>(() =>
            second.CommitLeaseIfActiveAsync(
                operationLease!,
                CreateSignedLease(subject, firstRegistration.RegistrationId),
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task Throwing_cancellation_callback_cannot_change_revocation_outcome()
    {
        const string subject = "throwing-callback-subject";
        MemoryDeviceRegistry registry = new();
        DeviceRegistrationResponse registration = await registry.RegisterAsync(
            subject,
            new DeviceRegistrationRequest(
                Hash("throwing-callback-key"),
                "0.1.0-beta.1",
                "windows",
                "x64",
                "development-1"),
            TestContext.Current.CancellationToken);
        ActiveDeviceRegistration? operationLease = await registry.FindActiveAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken);
        Assert.NotNull(operationLease);
        using CancellationTokenRegistration callback = operationLease!.RevocationToken.Register(
            static () => throw new InvalidOperationException("test callback failure"));

        DeviceRevocationOutcome outcome = await registry.RevokeAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken);

        Assert.Equal(DeviceRevocationOutcome.Revoked, outcome);
        Assert.True(operationLease.RevocationToken.IsCancellationRequested);
    }

    [Fact]
    public async Task Blocking_cancellation_callback_cannot_delay_revocation_outcome()
    {
        const string subject = "blocking-callback-subject";
        MemoryDeviceRegistry registry = new();
        DeviceRegistrationResponse registration = await registry.RegisterAsync(
            subject,
            new DeviceRegistrationRequest(
                Hash("blocking-callback-key"),
                "0.1.0-beta.1",
                "windows",
                "x64",
                "development-1"),
            TestContext.Current.CancellationToken);
        ActiveDeviceRegistration? operationLease = await registry.FindActiveAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken);
        Assert.NotNull(operationLease);
        TaskCompletionSource<bool> callbackStarted = new(
            TaskCreationOptions.RunContinuationsAsynchronously);
        TaskCompletionSource<bool> releaseCallback = new(
            TaskCreationOptions.RunContinuationsAsynchronously);
        using CancellationTokenRegistration callback = operationLease!.RevocationToken.Register(
            () =>
            {
                callbackStarted.TrySetResult(true);
                releaseCallback.Task.GetAwaiter().GetResult();
            });

        Task<DeviceRevocationOutcome> revocation = Task.Run(() => registry.RevokeAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken));
        await callbackStarted.Task.WaitAsync(TestContext.Current.CancellationToken);
        Task firstCompletion = await Task.WhenAny(
            revocation,
            Task.Delay(TimeSpan.FromSeconds(2), TestContext.Current.CancellationToken));
        bool completedBeforeCallback = ReferenceEquals(firstCompletion, revocation);
        releaseCallback.TrySetResult(true);

        Assert.True(completedBeforeCallback);
        Assert.Equal(
            DeviceRevocationOutcome.Revoked,
            await revocation.WaitAsync(TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task Revocation_cancels_an_in_flight_entitlement_lease()
    {
        BlockingSignedPolicyService signer = new();
        await using SupportApiFactory factory = new(signedPolicyService: signer);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);

        Task<HttpResponseMessage> leaseTask = SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-in-flight-lease");
        await signer.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage revoke = await SendAsync(
            client,
            HttpMethod.Delete,
            $"/desktop/v1/devices/registrations/{registration.RegistrationId}",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        using HttpResponseMessage lease = await leaseTask.WaitAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.NoContent, revoke.StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, lease.StatusCode);
        await signer.WaitUntilCancellationObservedAsync(TestContext.Current.CancellationToken);
        Assert.True(signer.CancellationObserved);
    }

    [Fact]
    public async Task Revocation_rejects_a_noncooperative_entitlement_lease()
    {
        NonCooperativeSignedPolicyService signer = new();
        await using SupportApiFactory factory = new(signedPolicyService: signer);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);

        Task<HttpResponseMessage> leaseTask = SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-noncooperative-lease");
        await signer.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage revoke = await SendAsync(
            client,
            HttpMethod.Delete,
            $"/desktop/v1/devices/registrations/{registration.RegistrationId}",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        signer.ReleaseCompletion();
        using HttpResponseMessage lease = await leaseTask.WaitAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.NoContent, revoke.StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, lease.StatusCode);
    }

    [Fact]
    public async Task Connected_entitlement_operation_has_a_server_owned_deadline()
    {
        BlockingSignedPolicyService signer = new();
        await using SupportApiFactory factory = new(
            signedPolicyService: signer,
            connectedOperationTimeoutSeconds: 1);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);

        using HttpResponseMessage response = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-timeout-lease");

        Assert.Equal(HttpStatusCode.GatewayTimeout, response.StatusCode);
        await AssertSafeProblemAsync(
            response,
            "connected_operation_timeout",
            HttpStatusCode.GatewayTimeout);
        await signer.WaitUntilCancellationObservedAsync(TestContext.Current.CancellationToken);
    }

    [Fact]
    public async Task Noncooperative_entitlement_operation_cannot_outlive_the_server_deadline()
    {
        NonCooperativeSignedPolicyService signer = new();
        await using SupportApiFactory factory = new(
            signedPolicyService: signer,
            connectedOperationTimeoutSeconds: 1);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);

        Task<HttpResponseMessage> responseTask = SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-noncooperative-timeout-lease");
        await signer.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage response = await responseTask.WaitAsync(
            TimeSpan.FromSeconds(5),
            TestContext.Current.CancellationToken);
        Assert.Equal(HttpStatusCode.GatewayTimeout, response.StatusCode);
        await AssertSafeProblemAsync(
            response,
            "connected_operation_timeout",
            HttpStatusCode.GatewayTimeout);

        signer.ReleaseCompletion();
        using HttpResponseMessage retry = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-noncooperative-timeout-lease");
        Assert.Equal(HttpStatusCode.OK, retry.StatusCode);
        Assert.Equal(2, signer.CallCount);
    }

    [Fact]
    public async Task Noncooperative_policy_provider_cannot_outlive_the_server_deadline()
    {
        NonCooperativeSignedPolicyService signer = new();
        await using SupportApiFactory factory = new(
            signedPolicyService: signer,
            connectedOperationTimeoutSeconds: 1);
        using HttpClient client = factory.CreateSecureClient();

        Task<HttpResponseMessage> responseTask = SendAsync(
            client,
            HttpMethod.Get,
            "/desktop/v1/policy/current",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        await signer.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage response = await responseTask.WaitAsync(
            TimeSpan.FromSeconds(5),
            TestContext.Current.CancellationToken);
        Assert.Equal(HttpStatusCode.GatewayTimeout, response.StatusCode);
        await AssertSafeProblemAsync(
            response,
            "connected_operation_timeout",
            HttpStatusCode.GatewayTimeout);
        signer.ReleaseCompletion();
        await signer.WaitUntilCompletedAsync(TestContext.Current.CancellationToken);
    }

    [Fact]
    public async Task Revocation_cancels_in_flight_model_access_before_a_receipt_is_stored()
    {
        BlockingModelAccessBroker broker = new();
        await using SupportApiFactory factory = new(modelAccessBroker: broker);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");

        Task<HttpResponseMessage> modelTask = SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-in-flight-model");
        await broker.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage revoke = await SendAsync(
            client,
            HttpMethod.Delete,
            $"/desktop/v1/devices/registrations/{registration.RegistrationId}",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        using HttpResponseMessage model = await modelTask.WaitAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.NoContent, revoke.StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, model.StatusCode);
        await broker.WaitUntilCancellationObservedAsync(TestContext.Current.CancellationToken);
        Assert.True(broker.CancellationObserved);
    }

    [Fact]
    public async Task Revocation_rejects_a_noncooperative_model_result_before_receipt_commit()
    {
        NonCooperativeModelAccessBroker broker = new();
        MemoryDeviceRegistry registry = new();
        await using SupportApiFactory factory = new(
            modelAccessBroker: broker,
            deviceRegistry: registry);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");

        Task<HttpResponseMessage> modelTask = SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-noncooperative-model");
        await broker.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage revoke = await SendAsync(
            client,
            HttpMethod.Delete,
            $"/desktop/v1/devices/registrations/{registration.RegistrationId}",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        broker.ReleaseCompletion();
        using HttpResponseMessage model = await modelTask.WaitAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.NoContent, revoke.StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, model.StatusCode);
        ModelAccessResult lateResult = await broker.WaitUntilCompletedAsync(
            TestContext.Current.CancellationToken);
        Assert.Null(await registry.GetReceiptAsync(
            SubjectPartition(SubjectA),
            lateResult.Receipt.ReceiptId,
            TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task Connected_model_operation_has_a_server_owned_deadline()
    {
        BlockingModelAccessBroker broker = new();
        await using SupportApiFactory factory = new(
            modelAccessBroker: broker,
            connectedOperationTimeoutSeconds: 1);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");

        using HttpResponseMessage response = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-timeout-model");

        Assert.Equal(HttpStatusCode.GatewayTimeout, response.StatusCode);
        await AssertSafeProblemAsync(
            response,
            "connected_operation_timeout",
            HttpStatusCode.GatewayTimeout);
        await broker.WaitUntilCancellationObservedAsync(TestContext.Current.CancellationToken);
    }

    [Fact]
    public async Task Noncooperative_model_operation_cannot_publish_after_the_server_deadline()
    {
        NonCooperativeModelAccessBroker broker = new();
        MemoryModelCallIdempotencyStore modelCalls = new(
            32,
            TimeSpan.FromMinutes(15),
            TimeProvider.System);
        MemoryDeviceRegistry registry = new();
        await using SupportApiFactory factory = new(
            modelAccessBroker: broker,
            modelCallIdempotencyStore: modelCalls,
            deviceRegistry: registry,
            connectedOperationTimeoutSeconds: 1);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");

        Task<HttpResponseMessage> responseTask = SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-noncooperative-timeout-model");
        await broker.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage response = await responseTask.WaitAsync(
            TimeSpan.FromSeconds(5),
            TestContext.Current.CancellationToken);
        Assert.Equal(HttpStatusCode.GatewayTimeout, response.StatusCode);
        await AssertSafeProblemAsync(
            response,
            "connected_operation_timeout",
            HttpStatusCode.GatewayTimeout);
        Assert.Equal(0, modelCalls.RetainedPayloadTaskCount);

        broker.ReleaseCompletion();
        ModelAccessResult lateResult = await broker.WaitUntilCompletedAsync(
            TestContext.Current.CancellationToken);
        Assert.Null(await registry.GetReceiptAsync(
            SubjectPartition(SubjectA),
            lateResult.Receipt.ReceiptId,
            TestContext.Current.CancellationToken));

        using HttpResponseMessage retry = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-noncooperative-timeout-model");
        Assert.Equal(HttpStatusCode.Conflict, retry.StatusCode);
        await AssertSafeProblemAsync(
            retry,
            "consent_already_consumed",
            HttpStatusCode.Conflict);
        Assert.Equal(1, broker.CallCount);
    }

    [Fact]
    public async Task Noncooperative_consent_verifier_cannot_outlive_the_server_deadline()
    {
        NonCooperativeContextConsentVerifier consentVerifier = new();
        CountingModelAccessBroker broker = new("the broker must not be called");
        await using SupportApiFactory factory = new(
            modelAccessBroker: broker,
            contextConsentVerifier: consentVerifier,
            connectedOperationTimeoutSeconds: 1);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");

        Task<HttpResponseMessage> responseTask = SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-noncooperative-timeout-consent");
        await consentVerifier.WaitUntilStartedAsync(TestContext.Current.CancellationToken);

        using HttpResponseMessage response = await responseTask.WaitAsync(
            TimeSpan.FromSeconds(5),
            TestContext.Current.CancellationToken);
        Assert.Equal(HttpStatusCode.GatewayTimeout, response.StatusCode);
        await AssertSafeProblemAsync(
            response,
            "connected_operation_timeout",
            HttpStatusCode.GatewayTimeout);
        Assert.Equal(0, broker.CallCount);

        consentVerifier.ReleaseCompletion();
        await consentVerifier.WaitUntilCompletedAsync(TestContext.Current.CancellationToken);
        Assert.Equal(0, broker.CallCount);
    }

    [Fact]
    public async Task Generic_idempotency_retention_starts_when_the_operation_completes()
    {
        ManualTimeProvider time = new(DateTimeOffset.Parse("2026-07-13T12:00:00Z"));
        MemoryIdempotencyStore idempotency = new(
            8,
            TimeSpan.FromMinutes(1),
            time);
        TaskCompletionSource<string> completion = new(
            TaskCreationOptions.RunContinuationsAsynchronously);
        int callCount = 0;

        Task<string> first = idempotency.ExecuteAsync(
            "subject",
            "idem-retention-from-completion",
            "request-fingerprint",
            () =>
            {
                Interlocked.Increment(ref callCount);
                return completion.Task;
            },
            TestContext.Current.CancellationToken);
        time.Advance(TimeSpan.FromMinutes(2));
        completion.TrySetResult("first-result");
        Assert.Equal("first-result", await first);

        string replay = await idempotency.ExecuteAsync(
            "subject",
            "idem-retention-from-completion",
            "request-fingerprint",
            () =>
            {
                Interlocked.Increment(ref callCount);
                return Task.FromResult("second-result");
            },
            TestContext.Current.CancellationToken);

        Assert.Equal("first-result", replay);
        Assert.Equal(1, callCount);
    }

    [Fact]
    public async Task Receipt_commit_before_revocation_is_durable_and_stales_the_operation_lease()
    {
        const string subject = "receipt-commit-subject";
        MemoryDeviceRegistry registry = new();
        DeviceRegistrationResponse registration = await registry.RegisterAsync(
            subject,
            new DeviceRegistrationRequest(
                Hash("receipt-commit-key"),
                "0.1.0-beta.1",
                "windows",
                "x64",
                "development-1"),
            TestContext.Current.CancellationToken);
        ActiveDeviceRegistration? operationLease = await registry.FindActiveAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken);
        Assert.NotNull(operationLease);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");
        ModelAccessResult result = CreateModelResult(request);

        ModelAccessResult committed = await registry.CommitModelResultIfActiveAsync(
            operationLease!,
            request,
            result,
            "test-region",
            TestContext.Current.CancellationToken);
        DeviceRevocationOutcome revocation = await registry.RevokeAsync(
            subject,
            registration.RegistrationId,
            TestContext.Current.CancellationToken);

        Assert.Equal(result, committed);
        Assert.Equal(DeviceRevocationOutcome.Revoked, revocation);
        Assert.Equal(
            result.Receipt,
            await registry.GetReceiptAsync(
                subject,
                result.Receipt.ReceiptId,
                TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<DeviceRegistrationRevokedException>(() =>
            registry.CommitModelResultIfActiveAsync(
                operationLease!,
                request,
                CreateModelResult(request),
                "test-region",
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task Model_receipt_identifier_is_unique_for_each_distinct_commit()
    {
        const string subject = "receipt-uniqueness-subject";
        MemoryDeviceRegistry registry = new();
        DeviceRegistrationResponse registration = await registry.RegisterAsync(
            subject,
            new DeviceRegistrationRequest(
                Hash("receipt-uniqueness-key"),
                "0.1.0-beta.1",
                "windows",
                "x64",
                "policy-v1"),
            TestContext.Current.CancellationToken);
        ActiveDeviceRegistration operationLease = Assert.IsType<ActiveDeviceRegistration>(
            await registry.FindActiveAsync(
                subject,
                registration.RegistrationId,
                TestContext.Current.CancellationToken));
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");
        ModelAccessResult first = CreateModelResult(request);

        await registry.CommitModelResultIfActiveAsync(
            operationLease,
            request,
            first,
            "test-region",
            TestContext.Current.CancellationToken);
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            registry.CommitModelResultIfActiveAsync(
                operationLease,
                request,
                first,
                "test-region",
                TestContext.Current.CancellationToken));

        ModelAccessResult different = CreateModelResult(
            request,
            "{\"summary\":\"a different result\"}");
        different = different with
        {
            Receipt = different.Receipt with { ReceiptId = first.Receipt.ReceiptId },
        };
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            registry.CommitModelResultIfActiveAsync(
                operationLease,
                request,
                different,
                "test-region",
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public async Task Malformed_model_results_are_rejected_before_receipt_publication()
    {
        const string subject = "malformed-result-subject";
        MemoryDeviceRegistry registry = new();
        DeviceRegistrationResponse registration = await registry.RegisterAsync(
            subject,
            new DeviceRegistrationRequest(
                Hash("malformed-result-key"),
                "0.1.0-beta.1",
                "windows",
                "x64",
                "policy-v1"),
            TestContext.Current.CancellationToken);
        ActiveDeviceRegistration operationLease = Assert.IsType<ActiveDeviceRegistration>(
            await registry.FindActiveAsync(
                subject,
                registration.RegistrationId,
                TestContext.Current.CancellationToken));
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");
        ModelAccessResult valid = CreateModelResult(request);
        ModelAccessReceipt receipt = valid.Receipt;
        ModelAccessResult[] malformed =
        [
            valid with { SchemaVersion = "desktop-model-access-result.v2" },
            valid with { RequestId = "different-request" },
            valid with { OutputSchemaId = "https://schemas.sapphirus.invalid/other" },
            valid with { PayloadJson = "{\"summary\":\"changed\"}" },
            valid with { PayloadJson = "{\"summary\":1,\"summary\":2}" },
            valid with { PayloadJson = "not-json" },
            valid with { PayloadHash = Hash("different-payload") },
            valid with { Receipt = receipt with { SchemaVersion = "desktop-model-access-receipt.v2" } },
            valid with { Receipt = receipt with { ReceiptId = "invalid-receipt" } },
            valid with { Receipt = receipt with { RequestHash = Hash("different-request") } },
            valid with { Receipt = receipt with { ResultHash = Hash("different-result") } },
            valid with { Receipt = receipt with { ManifestHash = Hash("different-manifest") } },
            valid with { Receipt = receipt with { ConsentEnvelopeHash = Hash("different-consent") } },
            valid with { Receipt = receipt with { ModelProfileHash = "invalid-profile-hash" } },
            valid with { Receipt = receipt with { RetentionMode = "stored" } },
            valid with { Receipt = receipt with { Region = "different-region" } },
            valid with { Receipt = receipt with { InputBytes = receipt.InputBytes + 1 } },
            valid with { Receipt = receipt with { OutputBytes = receipt.OutputBytes + 1 } },
            valid with
            {
                Receipt = receipt with
                {
                    StartedAt = receipt.CompletedAt.AddSeconds(1),
                },
            },
            valid with { Receipt = receipt with { TerminalStatus = "failed" } },
        ];

        foreach (ModelAccessResult result in malformed)
        {
            await Assert.ThrowsAsync<InvalidOperationException>(() =>
                registry.CommitModelResultIfActiveAsync(
                    operationLease,
                    request,
                    result,
                    "test-region",
                    TestContext.Current.CancellationToken));
            Assert.Null(await registry.GetReceiptAsync(
                subject,
                result.Receipt.ReceiptId,
                TestContext.Current.CancellationToken));
        }
    }

    [Fact]
    public async Task Completed_model_call_replay_returns_only_a_safe_marker()
    {
        const string privacyCanary = "MODEL_RESPONSE_MUST_NOT_BE_RETAINED_6b21959c";
        CountingModelAccessBroker broker = new(privacyCanary);
        MemoryModelCallIdempotencyStore modelCalls = new(
            32,
            TimeSpan.FromMinutes(15),
            TimeProvider.System);
        await using SupportApiFactory factory = new(
            modelAccessBroker: broker,
            modelCallIdempotencyStore: modelCalls);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");

        using HttpResponseMessage first = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-completed-model-marker");
        string firstBody = await first.Content.ReadAsStringAsync(
            TestContext.Current.CancellationToken);
        using HttpResponseMessage replay = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-completed-model-marker");
        string replayBody = await replay.Content.ReadAsStringAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Contains(privacyCanary, firstBody, StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.Conflict, replay.StatusCode);
        Assert.DoesNotContain(privacyCanary, replayBody, StringComparison.Ordinal);
        using JsonDocument replayProblem = JsonDocument.Parse(replayBody);
        Assert.Equal(
            "model_call_already_completed",
            replayProblem.RootElement.GetProperty("code").GetString());
        Assert.Equal(1, broker.CallCount);
        Assert.Equal(0, modelCalls.RetainedPayloadTaskCount);
    }

    [Fact]
    public async Task Revocation_precedes_delivery_of_a_completed_model_call_marker()
    {
        CountingModelAccessBroker broker = new("completed before revocation");
        await using SupportApiFactory factory = new(modelAccessBroker: broker);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");

        using HttpResponseMessage first = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-revoked-completed-model");
        using HttpResponseMessage revoke = await SendAsync(
            client,
            HttpMethod.Delete,
            $"/desktop/v1/devices/registrations/{registration.RegistrationId}",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        using HttpResponseMessage replay = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-revoked-completed-model");

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.NoContent, revoke.StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, replay.StatusCode);
        await AssertSafeProblemAsync(
            replay,
            "device_registration_unavailable",
            HttpStatusCode.Forbidden);
        Assert.Equal(1, broker.CallCount);
    }

    [Fact]
    public async Task Model_access_recomputes_content_and_manifest_hashes()
    {
        await using SupportApiFactory factory = new(consentVerified: true);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest valid = CreateModelRequest(registration.RegistrationId, "review this file");

        using HttpResponseMessage accepted = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            valid,
            "idem-model-valid");
        Assert.Equal(HttpStatusCode.OK, accepted.StatusCode);

        ModelContextItem original = valid.Items[0];
        ModelAccessRequest changedBytes = valid with
        {
            Items = [original with { Content = "review that file" }],
        };
        using HttpResponseMessage contentDrift = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            changedBytes,
            "idem-model-content-drift");
        Assert.Equal(HttpStatusCode.BadRequest, contentDrift.StatusCode);

        ModelAccessRequest manifestDrift = valid with { LocalEgressManifestHash = ValidHash };
        using HttpResponseMessage changedManifest = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            manifestDrift,
            "idem-model-manifest-drift");
        Assert.Equal(HttpStatusCode.BadRequest, changedManifest.StatusCode);
    }

    [Fact]
    public async Task Verified_consent_can_be_consumed_only_once_across_idempotency_keys()
    {
        TestContextConsentConsumptionStore consumptionStore = new();
        await using SupportApiFactory factory = new(consentConsumptionStore: consumptionStore);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(registration.RegistrationId, "review this file");

        using HttpResponseMessage first = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-consent-first");
        using HttpResponseMessage replay = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-consent-second");

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.Conflict, replay.StatusCode);
        await AssertSafeProblemAsync(replay, "consent_already_consumed", HttpStatusCode.Conflict);
    }

    [Fact]
    public async Task Duplicate_nested_json_members_are_rejected()
    {
        await using SupportApiFactory factory = new(consentVerified: true);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(
            registration.RegistrationId,
            "review this file");
        string body = JsonSerializer.Serialize(
            request,
            new JsonSerializerOptions(JsonSerializerDefaults.Web));
        const string contentMember = "\"content\":\"review this file\"";
        string duplicateBody = body.Replace(
            contentMember,
            $"{contentMember},{contentMember}",
            StringComparison.Ordinal);
        Assert.NotEqual(body, duplicateBody);

        using HttpResponseMessage response = await SendRawJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            duplicateBody,
            "idem-duplicate-context-member");

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        await AssertSafeProblemAsync(response, "request_invalid", HttpStatusCode.BadRequest);
    }

    [Theory]
    [InlineData("/src/example.cs")]
    [InlineData("src\\example.cs")]
    [InlineData("C:/src/example.cs")]
    [InlineData("src/../example.cs")]
    public async Task Model_access_rejects_non_relative_context_labels(string relativeLabel)
    {
        await using SupportApiFactory factory = new(consentVerified: true);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(registration.RegistrationId, "review this file");
        request = request with
        {
            Items = [request.Items[0] with { RelativeLabel = relativeLabel }],
        };

        using HttpResponseMessage response = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-unsafe-context-label");
        using JsonDocument problem = JsonDocument.Parse(
            await response.Content.ReadAsStringAsync(TestContext.Current.CancellationToken));

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        Assert.Equal(
            "context_item_invalid",
            problem.RootElement.GetProperty("code").GetString());
    }

    [Fact]
    public void Context_manifest_hash_matches_the_purpose_separated_golden_vector()
    {
        ModelContextItem item = new(
            "context-item-1",
            "src/example.txt",
            "implementation",
            "text",
            "sha256:5c2747dcba9b399166829e0058228130e5732dbfa9c77a176cbf5bfe8ca4b46e",
            8,
            "source",
            "hello π");
        ModelAccessRequest request = new(
            "desktop-model-access-request.v1",
            "request_12345678",
            "windows_local",
            "dreg_aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "plan_changes",
            "planning",
            "https://schemas.sapphirus.invalid/model-plan.v1",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ValidHash,
            CreateModelRequest("dreg_aaaaaaaaaaaaaaaaaaaaaaaaaa", "hello Ï€").Consent,
            [item],
            "transient_no_store",
            "interactive");

        Assert.Equal(
            "sha256:7cd6071668c47dc530620c59e21e6116ebe4746b7e262039d3f76f43dc06c3ec",
            RequestGuards.ComputeContextManifestHash(request));
    }

    [Fact]
    public async Task Opaque_consent_hash_is_not_treated_as_verified()
    {
        await using SupportApiFactory factory = new(consentVerified: false);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(registration.RegistrationId, "review this file");

        using HttpResponseMessage response = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-consent-unavailable");

        Assert.Equal(HttpStatusCode.ServiceUnavailable, response.StatusCode);
    }

    [Fact]
    public async Task Configured_release_channel_is_projected()
    {
        await using SupportApiFactory factory = new(releaseChannel: "stable");
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            HttpMethod.Get,
            "/desktop/v1/releases/current",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        ReleaseResponse? release = await response.Content.ReadFromJsonAsync<ReleaseResponse>(
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.NotNull(release);
        Assert.Equal("stable", release!.Channel);
    }

    [Fact]
    public async Task Failed_idempotent_operation_is_removed_and_can_be_retried()
    {
        MemoryIdempotencyStore store = new(4, TimeSpan.FromMinutes(10), TimeProvider.System);
        int attempts = 0;

        await Assert.ThrowsAsync<InvalidOperationException>(() => store.ExecuteAsync(
            "subject",
            "idem-failure-retry",
            "fingerprint",
            () =>
            {
                attempts++;
                return Task.FromException<int>(new InvalidOperationException("expected"));
            },
            TestContext.Current.CancellationToken));
        int result = await store.ExecuteAsync(
            "subject",
            "idem-failure-retry",
            "fingerprint",
            () =>
            {
                attempts++;
                return Task.FromResult(42);
            },
            TestContext.Current.CancellationToken);

        Assert.Equal(42, result);
        Assert.Equal(2, attempts);
        Assert.Equal(1, store.EntryCount);
    }

    [Fact]
    public async Task Idempotency_capacity_never_evicts_an_in_flight_operation()
    {
        MemoryIdempotencyStore store = new(1, TimeSpan.FromMinutes(10), TimeProvider.System);
        TaskCompletionSource started = new(TaskCreationOptions.RunContinuationsAsynchronously);
        TaskCompletionSource release = new(TaskCreationOptions.RunContinuationsAsynchronously);
        Task<int> first = store.ExecuteAsync(
            "subject",
            "idem-first-in-flight",
            "fingerprint-1",
            async () =>
            {
                started.SetResult();
                await release.Task;
                return 1;
            },
            TestContext.Current.CancellationToken);
        await started.Task;

        await Assert.ThrowsAsync<IdempotencyCapacityException>(() => store.ExecuteAsync(
            "subject",
            "idem-second-in-flight",
            "fingerprint-2",
            () => Task.FromResult(2),
            TestContext.Current.CancellationToken));
        Assert.Equal(1, store.EntryCount);

        release.SetResult();
        Assert.Equal(1, await first);
    }

    [Fact]
    public async Task Completed_idempotency_entry_is_evicted_only_after_retention()
    {
        ManualTimeProvider time = new(new DateTimeOffset(2026, 7, 13, 12, 0, 0, TimeSpan.Zero));
        MemoryIdempotencyStore store = new(1, TimeSpan.FromMinutes(10), time);
        Assert.Equal(1, await store.ExecuteAsync(
            "subject",
            "idem-retained",
            "fingerprint-1",
            () => Task.FromResult(1),
            TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<IdempotencyCapacityException>(() => store.ExecuteAsync(
            "subject",
            "idem-before-expiry",
            "fingerprint-2",
            () => Task.FromResult(2),
            TestContext.Current.CancellationToken));

        time.Advance(TimeSpan.FromMinutes(11));
        Assert.Equal(2, await store.ExecuteAsync(
            "subject",
            "idem-after-expiry",
            "fingerprint-2",
            () => Task.FromResult(2),
            TestContext.Current.CancellationToken));
        Assert.Equal(1, store.EntryCount);
    }

    private static async Task<DeviceRegistrationResponse> RegisterAsync(
        HttpClient client,
        string subject)
    {
        DeviceRegistrationRequest request = new(
            Hash("installation-key"),
            "0.1.0-beta.1",
            "windows",
            "x64",
            "development-1");
        using HttpResponseMessage response = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/devices/registrations",
            subject,
            DesktopScopes.Access,
            request,
            "idem-register-" + subject[..8]);
        DeviceRegistrationResponse? registration =
            await response.Content.ReadFromJsonAsync<DeviceRegistrationResponse>();
        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.NotNull(registration);
        Assert.Matches(
            "^dreg_[0-9A-HJKMNP-TV-Z]{26}$",
            registration!.RegistrationId);
        return registration!;
    }

    private static ModelAccessRequest CreateModelRequest(string registrationId, string content)
    {
        ModelContextItem item = new(
            "context-item-1",
            "src/example.cs",
            "implementation",
            "csharp",
            Hash(content),
            Encoding.UTF8.GetByteCount(content),
            "source",
            content);
        DateTimeOffset now = DateTimeOffset.UtcNow;
        ModelContextConsent consent = new(
            "sapphirus.model-context-consent.v1",
            "decision_12345678",
            "request_12345678",
            "invoke_12345678",
            "windows_local",
            Hash("tenant"),
            Hash("subject"),
            registrationId,
            Hash("installation-key"),
            "lease_12345678",
            Hash("entitlement-lease"),
            "policy_12345678",
            1,
            Hash("tenant-policy"),
            "plan_changes",
            "planning",
            "https://schemas.sapphirus.invalid/model-plan.v1",
            Hash("model-plan-schema"),
            ValidHash,
            Hash("invocation-binding"),
            Hash("consumption"),
            Hash("consent-disclosure"),
            Hash("provider-profile"),
            Hash("model-profile"),
            Hash("model-capability"),
            Hash("deployment"),
            "test-region",
            "transient_no_store",
            "interactive",
            now,
            now,
            now.AddMinutes(5),
            Hash("consent-nonce"),
            Hash("consent-envelope"),
            new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                "test-installation-key",
                Hash("consent-envelope"),
                "ZXhhbXBsZS1kZXZpY2Utc2lnbmF0dXJl"));
        ModelAccessRequest unbound = new(
            "desktop-model-access-request.v1",
            "request_12345678",
            "windows_local",
            registrationId,
            "plan_changes",
            "planning",
            "https://schemas.sapphirus.invalid/model-plan.v1",
            Hash("model-plan-schema"),
            ValidHash,
            consent,
            [item],
            "transient_no_store",
            "interactive");
        return unbound with
        {
            LocalEgressManifestHash = RequestGuards.ComputeContextManifestHash(unbound),
            Consent = consent with
            {
                ManifestHash = RequestGuards.ComputeContextManifestHash(unbound),
            },
        };
    }

    private static Task<HttpResponseMessage> SendJsonAsync<T>(
        HttpClient client,
        HttpMethod method,
        string path,
        string subject,
        string scope,
        T body,
        string idempotencyKey)
    {
        HttpRequestMessage request = CreateRequest(
            method,
            path,
            subject,
            ApprovedClientId,
            scope);
        request.Headers.Add("Idempotency-Key", idempotencyKey);
        request.Content = JsonContent.Create(body);
        return client.SendAsync(request, TestContext.Current.CancellationToken);
    }

    private static Task<HttpResponseMessage> SendRawJsonAsync(
        HttpClient client,
        HttpMethod method,
        string path,
        string subject,
        string scope,
        string body,
        string idempotencyKey)
    {
        HttpRequestMessage request = CreateRequest(
            method,
            path,
            subject,
            ApprovedClientId,
            scope);
        request.Headers.Add("Idempotency-Key", idempotencyKey);
        request.Content = new StringContent(body, Encoding.UTF8, "application/json");
        return client.SendAsync(request, TestContext.Current.CancellationToken);
    }

    private static Task<HttpResponseMessage> SendAsync(
        HttpClient client,
        HttpMethod method,
        string path,
        string subject,
        string clientId,
        string scope) => client.SendAsync(
            CreateRequest(method, path, subject, clientId, scope),
            TestContext.Current.CancellationToken);

    private static HttpRequestMessage CreateRequest(
        HttpMethod method,
        string path,
        string subject,
        string clientId,
        string scope)
    {
        HttpRequestMessage request = new(method, path);
        request.Headers.Add(TestAuthenticationHandler.TenantHeader, TenantId);
        request.Headers.Add(TestAuthenticationHandler.SubjectHeader, subject);
        request.Headers.Add(TestAuthenticationHandler.ClientHeader, clientId);
        request.Headers.Add(TestAuthenticationHandler.ScopeHeader, scope);
        return request;
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));

    private static async Task AssertSafeProblemAsync(
        HttpResponseMessage response,
        string expectedCode,
        HttpStatusCode expectedStatus)
    {
        using JsonDocument problem = JsonDocument.Parse(
            await response.Content.ReadAsStringAsync(TestContext.Current.CancellationToken));
        JsonElement root = problem.RootElement;
        Assert.Equal(expectedCode, root.GetProperty("code").GetString());
        Assert.Equal((int)expectedStatus, root.GetProperty("status").GetInt32());
        Assert.Equal(4, root.EnumerateObject().Count());
    }

    private static string SubjectPartition(string subject)
    {
        ClaimsIdentity identity = new(
        [
            new Claim("tid", TenantId),
            new Claim("oid", subject),
        ],
        "SupportApiTests");
        return ScopeAuthorization.SubjectPartition(new ClaimsPrincipal(identity));
    }

    private static ModelAccessResult CreateModelResult(
        ModelAccessRequest request,
        string payload = "{\"summary\":\"completed\"}")
    {
        DateTimeOffset now = DateTimeOffset.UtcNow;
        string receiptHash = Hash($"receipt:{request.RequestId}:{payload}");
        ModelAccessReceipt receipt = new(
            "sapphirus.model-access-receipt.v1",
            "receipt_" + Guid.NewGuid().ToString("N").ToUpperInvariant(),
            request.RequestId,
            RequestGuards.Fingerprint(request),
            Hash(payload),
            "windows_local",
            request.Consent.TenantHash,
            request.Consent.SubjectHash,
            request.RegistrationId,
            request.LocalEgressManifestHash,
            request.Consent.InvocationBindingHash,
            request.Consent.ConsumptionHash,
            request.Consent.ConsentEnvelopeHash,
            request.Consent.ConsentDisclosureHash,
            request.Consent.ProviderProfileHash,
            request.Consent.ModelProfileHash,
            request.Consent.ModelCapabilityHash,
            request.Consent.DeploymentHash,
            request.CanonicalOutputSchemaId,
            request.CanonicalOutputSchemaHash,
            Hash("schema-projection"),
            Hash("credential-binding"),
            "transient_no_store",
            "test-region",
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(payload),
            new ModelAccessUsage(0, 0, 0, "EUR"),
            0,
            [],
            null,
            now,
            now,
            "succeeded",
            receiptHash,
            new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                "https://support.test.invalid/",
                "sapphirus-desktop-tests",
                "test-model-receipt-key",
                receiptHash,
                "dGVzdC1zdXBwb3J0LXBsYW5lLXNpZ25hdHVyZQ"));
        return new ModelAccessResult(
            "desktop-model-access-result.v1",
            request.RequestId,
            request.CanonicalOutputSchemaId,
            payload,
            Hash(payload),
            receipt);
    }

    private static SignedEntitlementLease CreateSignedLease(
        string subject,
        string registrationId)
    {
        DateTimeOffset now = DateTimeOffset.UtcNow;
        return new SignedEntitlementLease(
            "desktop-entitlement-lease.v1",
            "lease_" + Guid.NewGuid().ToString("N"),
            registrationId,
            Hash(subject),
            "windows_local",
            now,
            now.AddMinutes(-2),
            now.AddHours(24),
            now.AddHours(96),
            ["local_runtime", "model_access"],
            Hash("development-policy"),
            "0.1.0-beta.1",
            "test-key",
            "test-signature");
    }

    private sealed class SupportApiFactory(
        bool consentVerified = true,
        string releaseChannel = "beta",
        IModelAccessBroker? modelAccessBroker = null,
        ISignedPolicyService? signedPolicyService = null,
        IContextConsentVerifier? contextConsentVerifier = null,
        IContextConsentConsumptionStore? consentConsumptionStore = null,
        IModelCallIdempotencyStore? modelCallIdempotencyStore = null,
        IDeviceRegistry? deviceRegistry = null,
        int connectedOperationTimeoutSeconds = 120) : WebApplicationFactory<Program>
    {
        protected override void ConfigureWebHost(IWebHostBuilder builder)
        {
            builder.UseEnvironment("Development");
            builder.ConfigureLogging(logging => logging.ClearProviders());
            builder.UseSetting("Sapphirus:Authority", $"https://login.microsoftonline.com/{TenantId}/v2.0");
            builder.UseSetting("Sapphirus:Audience", "api://66666666-6666-6666-6666-666666666666");
            builder.UseSetting("Sapphirus:ApprovedDesktopClientId", ApprovedClientId);
            builder.UseSetting("Sapphirus:Region", "test-region");
            builder.UseSetting("Sapphirus:ReleaseChannel", releaseChannel);
            builder.UseSetting("Sapphirus:DevelopmentSigningEnabled", "true");
            builder.UseSetting("Sapphirus:DevelopmentModelEnabled", "true");
            builder.UseSetting(
                "Sapphirus:ConnectedOperationTimeoutSeconds",
                connectedOperationTimeoutSeconds.ToString());
            builder.ConfigureTestServices(services =>
            {
                services.AddAuthentication(authentication =>
                    {
                        authentication.DefaultAuthenticateScheme = TestAuthenticationHandler.AuthenticationSchemeName;
                        authentication.DefaultChallengeScheme = TestAuthenticationHandler.AuthenticationSchemeName;
                        authentication.DefaultForbidScheme = TestAuthenticationHandler.AuthenticationSchemeName;
                    })
                    .AddScheme<AuthenticationSchemeOptions, TestAuthenticationHandler>(
                        TestAuthenticationHandler.AuthenticationSchemeName,
                        _ => { });
                if (contextConsentVerifier is not null)
                {
                    services.RemoveAll<IContextConsentVerifier>();
                    services.AddSingleton(contextConsentVerifier);
                }
                else if (consentVerified)
                {
                    services.RemoveAll<IContextConsentVerifier>();
                    services.AddSingleton<IContextConsentVerifier, TestContextConsentVerifier>();
                }
                if (modelAccessBroker is not null)
                {
                    services.RemoveAll<IModelAccessBroker>();
                    services.AddSingleton(modelAccessBroker);
                }
                if (consentConsumptionStore is not null)
                {
                    services.RemoveAll<IContextConsentConsumptionStore>();
                    services.AddSingleton(consentConsumptionStore);
                }
                else if (consentVerified)
                {
                    services.RemoveAll<IContextConsentConsumptionStore>();
                    services.AddSingleton<IContextConsentConsumptionStore, TestContextConsentConsumptionStore>();
                }
                if (signedPolicyService is not null)
                {
                    services.RemoveAll<ISignedPolicyService>();
                    services.AddSingleton(signedPolicyService);
                }
                if (modelCallIdempotencyStore is not null)
                {
                    services.RemoveAll<IModelCallIdempotencyStore>();
                    services.AddSingleton(modelCallIdempotencyStore);
                }
                if (deviceRegistry is not null)
                {
                    services.RemoveAll<IDeviceRegistry>();
                    services.AddSingleton(deviceRegistry);
                }
            });
        }

        public HttpClient CreateSecureClient() => CreateClient(new WebApplicationFactoryClientOptions
        {
            AllowAutoRedirect = false,
            BaseAddress = new Uri("https://localhost"),
        });
    }

    private sealed class TestContextConsentVerifier : IContextConsentVerifier
    {
        public ValueTask<ContextConsentVerification> VerifyAsync(
            ContextConsentVerificationRequest request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return ValueTask.FromResult(ContextConsentVerification.Verified);
        }
    }

    private sealed class TestContextConsentConsumptionStore : IContextConsentConsumptionStore
    {
        private readonly HashSet<string> _consumed = new(StringComparer.Ordinal);

        public ValueTask<ContextConsentConsumption> ConsumeAsync(
            ContextConsentConsumptionRequest request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            lock (_consumed)
            {
                return ValueTask.FromResult(
                    _consumed.Add(request.SubjectPartition + "\0" + request.ConsumptionHash)
                        ? ContextConsentConsumption.Consumed
                        : ContextConsentConsumption.AlreadyConsumed);
            }
        }
    }

    private sealed class BlockingModelAccessBroker : IModelAccessBroker
    {
        private readonly TaskCompletionSource<bool> _started =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<bool> _cancelled =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int _cancellationObserved;

        public bool CancellationObserved => Volatile.Read(ref _cancellationObserved) == 1;

        public Task WaitUntilStartedAsync(CancellationToken cancellationToken) =>
            _started.Task.WaitAsync(cancellationToken);

        public Task WaitUntilCancellationObservedAsync(CancellationToken cancellationToken) =>
            _cancelled.Task.WaitAsync(cancellationToken);

        public async Task<ModelAccessResult> CompleteAsync(
            string subject,
            ModelAccessRequest request,
            CancellationToken cancellationToken)
        {
            _started.TrySetResult(true);
            try
            {
                await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
                throw new InvalidOperationException("The blocking model broker unexpectedly resumed.");
            }
            catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
            {
                Interlocked.Exchange(ref _cancellationObserved, 1);
                _cancelled.TrySetResult(true);
                throw;
            }
        }
    }

    private sealed class NonCooperativeModelAccessBroker : IModelAccessBroker
    {
        private readonly TaskCompletionSource<bool> _started =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<bool> _release =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<ModelAccessResult> _completed =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int _callCount;
        private ModelAccessResult? _completedResult;

        public int CallCount => Volatile.Read(ref _callCount);
        public string? CompletedReceiptId =>
            Volatile.Read(ref _completedResult)?.Receipt.ReceiptId;

        public Task WaitUntilStartedAsync(CancellationToken cancellationToken) =>
            _started.Task.WaitAsync(cancellationToken);

        public void ReleaseCompletion() => _release.TrySetResult(true);

        public Task<ModelAccessResult> WaitUntilCompletedAsync(CancellationToken cancellationToken) =>
            _completed.Task.WaitAsync(cancellationToken);

        public async Task<ModelAccessResult> CompleteAsync(
            string subject,
            ModelAccessRequest request,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref _callCount);
            _started.TrySetResult(true);
            await _release.Task;
            ModelAccessResult result = CreateModelResult(request);
            Volatile.Write(ref _completedResult, result);
            _completed.TrySetResult(result);
            return result;
        }
    }

    private sealed class NonCooperativeContextConsentVerifier : IContextConsentVerifier
    {
        private readonly TaskCompletionSource<bool> _started =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<bool> _release =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<bool> _completed =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        public Task WaitUntilStartedAsync(CancellationToken cancellationToken) =>
            _started.Task.WaitAsync(cancellationToken);

        public Task WaitUntilCompletedAsync(CancellationToken cancellationToken) =>
            _completed.Task.WaitAsync(cancellationToken);

        public void ReleaseCompletion() => _release.TrySetResult(true);

        public async ValueTask<ContextConsentVerification> VerifyAsync(
            ContextConsentVerificationRequest request,
            CancellationToken cancellationToken)
        {
            _started.TrySetResult(true);
            await _release.Task;
            _completed.TrySetResult(true);
            return ContextConsentVerification.Verified;
        }
    }

    private sealed class CountingModelAccessBroker(string payload) : IModelAccessBroker
    {
        private int _callCount;

        public int CallCount => Volatile.Read(ref _callCount);

        public Task<ModelAccessResult> CompleteAsync(
            string subject,
            ModelAccessRequest request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            Interlocked.Increment(ref _callCount);
            return Task.FromResult(CreateModelResult(
                request,
                JsonSerializer.Serialize(new { summary = payload })));
        }
    }

    private sealed class BlockingSignedPolicyService : ISignedPolicyService
    {
        private readonly TaskCompletionSource<bool> _started =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<bool> _cancelled =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int _cancellationObserved;

        public bool CancellationObserved => Volatile.Read(ref _cancellationObserved) == 1;

        public Task WaitUntilStartedAsync(CancellationToken cancellationToken) =>
            _started.Task.WaitAsync(cancellationToken);

        public Task WaitUntilCancellationObservedAsync(CancellationToken cancellationToken) =>
            _cancelled.Task.WaitAsync(cancellationToken);

        public Task<SignedDesktopPolicy> CurrentPolicyAsync(CancellationToken cancellationToken) =>
            throw new InvalidOperationException("The policy endpoint is not used by this test double.");

        public async Task<SignedEntitlementLease> CreateLeaseAsync(
            string subject,
            string registrationId,
            CancellationToken cancellationToken)
        {
            _started.TrySetResult(true);
            try
            {
                await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
                throw new InvalidOperationException("The blocking lease signer unexpectedly resumed.");
            }
            catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
            {
                Interlocked.Exchange(ref _cancellationObserved, 1);
                _cancelled.TrySetResult(true);
                throw;
            }
        }
    }

    private sealed class NonCooperativeSignedPolicyService : ISignedPolicyService
    {
        private readonly TaskCompletionSource<bool> _started =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<bool> _release =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource<bool> _completed =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int _callCount;

        public int CallCount => Volatile.Read(ref _callCount);

        public Task WaitUntilStartedAsync(CancellationToken cancellationToken) =>
            _started.Task.WaitAsync(cancellationToken);

        public void ReleaseCompletion() => _release.TrySetResult(true);

        public Task WaitUntilCompletedAsync(CancellationToken cancellationToken) =>
            _completed.Task.WaitAsync(cancellationToken);

        public async Task<SignedDesktopPolicy> CurrentPolicyAsync(
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref _callCount);
            _started.TrySetResult(true);
            await _release.Task;
            _completed.TrySetResult(true);
            return new SignedDesktopPolicy(
                "desktop-policy.v1",
                "test-policy-v1",
                Hash("test-policy"),
                false,
                512 * 1024,
                64,
                ["test-region"],
                "test-key",
                "test-signature");
        }

        public async Task<SignedEntitlementLease> CreateLeaseAsync(
            string subject,
            string registrationId,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref _callCount);
            _started.TrySetResult(true);
            await _release.Task;
            return CreateSignedLease(subject, registrationId);
        }
    }

    private sealed class ManualTimeProvider(DateTimeOffset utcNow) : TimeProvider
    {
        private DateTimeOffset _utcNow = utcNow;

        public override DateTimeOffset GetUtcNow() => _utcNow;

        public void Advance(TimeSpan duration) => _utcNow += duration;
    }

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Sapphirus.DesktopSupportApi.Tests";
        public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }

    private sealed class TestAuthenticationHandler(
        IOptionsMonitor<AuthenticationSchemeOptions> options,
        ILoggerFactory logger,
        UrlEncoder encoder) : AuthenticationHandler<AuthenticationSchemeOptions>(options, logger, encoder)
    {
        public const string AuthenticationSchemeName = "SupportApiTests";
        public const string TenantHeader = "X-Test-Tenant";
        public const string SubjectHeader = "X-Test-Subject";
        public const string ClientHeader = "X-Test-Client";
        public const string ScopeHeader = "X-Test-Scope";

        protected override Task<AuthenticateResult> HandleAuthenticateAsync()
        {
            if (!Request.Headers.TryGetValue(TenantHeader, out var tenant)
                || !Request.Headers.TryGetValue(SubjectHeader, out var subject)
                || !Request.Headers.TryGetValue(ClientHeader, out var client)
                || !Request.Headers.TryGetValue(ScopeHeader, out var scope))
            {
                return Task.FromResult(AuthenticateResult.NoResult());
            }
            Claim[] claims =
            [
                new("tid", tenant.ToString()),
                new("oid", subject.ToString()),
                new("azp", client.ToString()),
                new("scp", scope.ToString()),
            ];
            ClaimsIdentity identity = new(claims, AuthenticationSchemeName);
            AuthenticationTicket ticket = new(new ClaimsPrincipal(identity), AuthenticationSchemeName);
            return Task.FromResult(AuthenticateResult.Success(ticket));
        }
    }
}
