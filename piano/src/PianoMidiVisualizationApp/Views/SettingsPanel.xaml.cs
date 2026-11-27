using System.ComponentModel;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

public partial class SettingsPanel : UserControl
{
    private MainViewModel? _subscribedVm;

    public SettingsPanel()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (_subscribedVm != null)
        {
            _subscribedVm.Settings.PropertyChanged -= OnSettingsPropertyChanged;
            _subscribedVm = null;
        }

        // Initialize PasswordBox with saved API key when DataContext is set,
        // and hot-reload audio whenever the selected soundbank changes.
        if (e.NewValue is MainViewModel vm)
        {
            _subscribedVm = vm;
            _subscribedVm.Settings.PropertyChanged += OnSettingsPropertyChanged;

            if (!string.IsNullOrEmpty(vm.Settings.AnthropicApiKey))
            {
                ApiKeyBox.Password = vm.Settings.AnthropicApiKey;
            }
        }
    }

    private void OnSettingsPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(SettingsViewModel.SoundFontPath) || _subscribedVm is null)
            return;

        string path = _subscribedVm.Settings.SoundFontPath;
        if (!string.IsNullOrWhiteSpace(path) && File.Exists(path))
        {
            if (_subscribedVm.IsAudioRunning || !string.IsNullOrWhiteSpace(_subscribedVm.Settings.SelectedAudioDriver))
            {
                _subscribedVm.StartAudioCommand.Execute(null);
            }
        }
        else if (string.IsNullOrWhiteSpace(path) && _subscribedVm.IsAudioRunning)
        {
            _subscribedVm.StopAudioCommand.Execute(null);
        }
    }

    private void ApiKeyBox_PasswordChanged(object sender, RoutedEventArgs e)
    {
        if (DataContext is MainViewModel vm)
        {
            vm.Settings.AnthropicApiKey = ApiKeyBox.Password;
        }
    }
}
