using System.Windows;
using System.Windows.Controls;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

public partial class SettingsPanel : UserControl
{
    public SettingsPanel()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        // Initialize PasswordBox with saved API key when DataContext is set
        if (e.NewValue is MainViewModel vm && !string.IsNullOrEmpty(vm.Settings.AnthropicApiKey))
        {
            ApiKeyBox.Password = vm.Settings.AnthropicApiKey;
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
